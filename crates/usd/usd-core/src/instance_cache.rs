//! Usd_InstanceCache - internal cache for instance information.
//!
//! Port of pxr/usd/usd/instanceCache.h/cpp
//!
//! Private helper object for computing and caching instance information
//! on a UsdStage. This object is responsible for keeping track of the
//! instanceable prim indexes and their corresponding prototypes.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use usd_pcp::PrimIndex;
use usd_sdf::Path;

use super::instance_key::InstanceKey;

// ============================================================================
// InstanceChanges
// ============================================================================

/// List of changes to prototype prims due to the discovery of new
/// or destroyed instanceable prim indexes.
///
/// Matches C++ `Usd_InstanceChanges`.
#[derive(Debug, Clone, Default)]
pub struct InstanceChanges {
    /// List of new prototype prims and their corresponding source prim indexes.
    pub new_prototype_prims: Vec<Path>,
    /// List of new prototype prim index paths.
    pub new_prototype_prim_indexes: Vec<Path>,
    /// List of prototype prims that have been changed to use a new source prim index.
    pub changed_prototype_prims: Vec<Path>,
    /// List of changed prototype prim index paths.
    pub changed_prototype_prim_indexes: Vec<Path>,
    /// List of prototype prims that no longer have any instances.
    pub dead_prototype_prims: Vec<Path>,
}

impl InstanceChanges {
    /// Appends changes from another InstanceChanges object.
    ///
    /// Matches C++ `AppendChanges(const Usd_InstanceChanges& c)`.
    pub fn append_changes(&mut self, other: &InstanceChanges) {
        self.new_prototype_prims
            .extend_from_slice(&other.new_prototype_prims);
        self.new_prototype_prim_indexes
            .extend_from_slice(&other.new_prototype_prim_indexes);
        self.changed_prototype_prims
            .extend_from_slice(&other.changed_prototype_prims);
        self.changed_prototype_prim_indexes
            .extend_from_slice(&other.changed_prototype_prim_indexes);
        self.dead_prototype_prims
            .extend_from_slice(&other.dead_prototype_prims);
    }
}

// ============================================================================
// InstanceCache
// ============================================================================

/// Private helper object for computing and caching instance information.
///
/// Matches C++ `Usd_InstanceCache`.
///
/// This object is responsible for keeping track of the instanceable prim indexes
/// and their corresponding prototypes.
pub struct InstanceCache {
    /// Map from instance key to prototype path.
    key_to_prototype: Mutex<HashMap<InstanceKey, Path>>,
    /// Map from prototype path to instance key.
    prototype_to_key: Mutex<HashMap<Path, InstanceKey>>,
    /// Map from source prim index path to prototype path.
    source_prim_index_to_prototype: Mutex<BTreeMap<Path, Path>>,
    /// Map from prototype path to source prim index path.
    prototype_to_source_index: Mutex<BTreeMap<Path, Path>>,
    /// Map from prototype path to list of prim index paths.
    prototype_to_prim_indexes: Mutex<BTreeMap<Path, Vec<Path>>>,
    /// Map from prim index path to prototype path.
    prim_index_to_prototype: Mutex<BTreeMap<Path, Path>>,
    /// Pending added prim indexes (key -> list of paths).
    pending_added_prim_indexes: Mutex<HashMap<InstanceKey, Vec<Path>>>,
    /// Pending removed prim indexes (key -> list of paths).
    pending_removed_prim_indexes: Mutex<HashMap<InstanceKey, Vec<Path>>>,
    /// Index of last prototype prim created. Used to create prototype prim names.
    last_prototype_index: Mutex<usize>,
}

impl InstanceCache {
    /// Creates a new instance cache.
    ///
    /// Matches C++ `Usd_InstanceCache()`.
    pub fn new() -> Self {
        Self {
            key_to_prototype: Mutex::new(HashMap::new()),
            prototype_to_key: Mutex::new(HashMap::new()),
            source_prim_index_to_prototype: Mutex::new(BTreeMap::new()),
            prototype_to_source_index: Mutex::new(BTreeMap::new()),
            prototype_to_prim_indexes: Mutex::new(BTreeMap::new()),
            prim_index_to_prototype: Mutex::new(BTreeMap::new()),
            pending_added_prim_indexes: Mutex::new(HashMap::new()),
            pending_removed_prim_indexes: Mutex::new(HashMap::new()),
            last_prototype_index: Mutex::new(0),
        }
    }

    /// Clears all cached instance/prototype state.
    pub fn clear(&self) {
        self.key_to_prototype.lock().expect("lock poisoned").clear();
        self.prototype_to_key.lock().expect("lock poisoned").clear();
        self.source_prim_index_to_prototype
            .lock()
            .expect("lock poisoned")
            .clear();
        self.prototype_to_source_index
            .lock()
            .expect("lock poisoned")
            .clear();
        self.prototype_to_prim_indexes
            .lock()
            .expect("lock poisoned")
            .clear();
        self.prim_index_to_prototype
            .lock()
            .expect("lock poisoned")
            .clear();
        self.pending_added_prim_indexes
            .lock()
            .expect("lock poisoned")
            .clear();
        self.pending_removed_prim_indexes
            .lock()
            .expect("lock poisoned")
            .clear();
        *self.last_prototype_index.lock().expect("lock poisoned") = 0;
    }

    /// Registers the given instance prim index with the cache.
    ///
    /// Matches C++ `RegisterInstancePrimIndex(const PcpPrimIndex& index, const UsdStagePopulationMask *mask, const UsdStageLoadRules &loadRules)`.
    ///
    /// The index will be added to a list of pending changes and will
    /// not take effect until a subsequent call to ProcessChanges.
    ///
    /// It is safe to call this function concurrently from multiple threads.
    ///
    /// Returns true if the given instance prim index requires a new
    /// prototype prim or is the source for an existing prototype prim, false
    /// otherwise.
    pub fn register_instance_prim_index(
        &self,
        index: &Arc<PrimIndex>,
        mask: Option<&crate::population_mask::StagePopulationMask>,
        load_rules: &crate::load_rules::StageLoadRules,
    ) -> bool {
        // Verify index is instanceable
        if !index.is_instanceable() {
            return false;
        }

        // Make sure we compute the key for this index before we grab
        // the mutex to minimize the time we hold the lock.
        let key = InstanceKey::from_prim_index(index, mask, load_rules);

        // Check whether a prototype for this prim index already exists
        // or if this prim index is already being used as the source for
        // a prototype.
        let key_to_prototype = self.key_to_prototype.lock().expect("lock poisoned");
        let prototype_already_exists = key_to_prototype.contains_key(&key);
        drop(key_to_prototype);

        let index_path = index.path().clone();
        let mut pending_added = self
            .pending_added_prim_indexes
            .lock()
            .expect("lock poisoned");
        let pending_indexes = pending_added.entry(key.clone()).or_default();
        pending_indexes.push(index_path.clone());

        // A new prototype must be created for this instance if one doesn't
        // already exist and this instance is the first one registered for
        // this key.
        let needs_new_prototype = !prototype_already_exists && pending_indexes.len() == 1;
        if needs_new_prototype {
            return true;
        }

        if prototype_already_exists {
            let key_to_prototype = self.key_to_prototype.lock().expect("lock poisoned");
            if let Some(prototype_path) = key_to_prototype.get(&key) {
                let prototype_to_source = self
                    .prototype_to_source_index
                    .lock()
                    .expect("lock poisoned");
                if let Some(source_path) = prototype_to_source.get(prototype_path) {
                    return *source_path == index_path.clone();
                }
            }
        }

        false
    }

    /// Unregisters all instance prim indexes at or under primIndexPath.
    ///
    /// Matches C++ `UnregisterInstancePrimIndexesUnder(const SdfPath& primIndexPath)`.
    ///
    /// The indexes will be added to a list of pending changes and will
    /// not take effect until a subsequent call to ProcessChanges.
    pub fn unregister_instance_prim_indexes_under(&self, prim_index_path: &Path) {
        let prim_index_to_prototype = self.prim_index_to_prototype.lock().expect("lock poisoned");
        let prototype_to_key = self.prototype_to_key.lock().expect("lock poisoned");
        let mut pending_removed = self
            .pending_removed_prim_indexes
            .lock()
            .expect("lock poisoned");

        // Find all prim indexes that are at or under prim_index_path
        for (index_path, prototype_path) in prim_index_to_prototype.iter() {
            if index_path.has_prefix(prim_index_path) {
                if let Some(key) = prototype_to_key.get(prototype_path) {
                    let pending_indexes = pending_removed.entry(key.clone()).or_default();
                    pending_indexes.push(index_path.clone());
                }
            }
        }
    }

    /// Process all instance prim indexes that have been registered or
    /// unregistered since the last call to this function and return the
    /// resulting list of prototype prim changes via changes.
    ///
    /// Matches C++ `ProcessChanges(Usd_InstanceChanges* changes)`.
    pub fn process_changes(&self, changes: &mut InstanceChanges) {
        // Remove unregistered prim indexes from the cache.
        let mut prototype_to_old_source_index_path: HashMap<Path, Path> = HashMap::new();

        let mut pending_removed = self
            .pending_removed_prim_indexes
            .lock()
            .expect("lock poisoned");
        let mut pending_added = self
            .pending_added_prim_indexes
            .lock()
            .expect("lock poisoned");

        // Process removals
        for (key, prim_indexes) in pending_removed.iter_mut() {
            // Ignore any unregistered prim index that was subsequently
            // re-registered.
            if let Some(registered) = pending_added.get(key) {
                let mut unregistered = std::mem::take(prim_indexes);
                let mut registered_sorted = registered.clone();
                registered_sorted.sort();
                unregistered.sort();

                // Compute set difference: unregistered - registered
                prim_indexes.clear();
                for path in unregistered {
                    if registered_sorted.binary_search(&path).is_err() {
                        prim_indexes.push(path);
                    }
                }
            }

            if !prim_indexes.is_empty() {
                self.remove_instances(
                    key,
                    prim_indexes,
                    changes,
                    &mut prototype_to_old_source_index_path,
                );
            }
        }

        // Add newly-registered prim indexes to the cache.
        // Process in deterministic order (sorted by first prim index path)
        let mut keys_to_process: Vec<(Path, InstanceKey)> = Vec::new();
        for (key, prim_indexes) in pending_added.iter() {
            if !prim_indexes.is_empty() {
                keys_to_process.push((prim_indexes[0].clone(), key.clone()));
            }
        }
        keys_to_process.sort_by(|a, b| a.0.cmp(&b.0));

        for (_, key) in keys_to_process {
            if let Some(prim_indexes) = pending_added.get_mut(&key) {
                self.create_or_update_prototype_for_instances(
                    &key,
                    prim_indexes,
                    changes,
                    &prototype_to_old_source_index_path,
                );
            }
        }

        // Now that we've processed all additions and removals, we can find and
        // drop any prototypes that have no instances associated with them.
        for (key, _) in pending_removed.iter() {
            self.remove_prototype_if_no_instances(key, changes);
        }

        pending_added.clear();
        pending_removed.clear();
    }

    /// Return true if path identifies a prototype or a prototype descendant.
    ///
    /// Matches C++ `IsPathInPrototype(const SdfPath& path)`.
    ///
    /// The path must be either an absolute path or empty.
    pub fn is_path_in_prototype(path: &Path) -> bool {
        if path.is_empty() || path == &Path::absolute_root() {
            return false;
        }
        if !path.is_absolute_path() {
            // We require an absolute path because there is no way for us
            // to walk to the root prim level from a relative path.
            return false;
        }

        // Walk up to root prim path
        let mut root_path = path.clone();
        while !root_path.is_root_prim_path() {
            root_path = root_path.get_parent_path();
        }

        root_path.as_str().starts_with("/__Prototype_")
    }

    /// Return true if path identifies a prototype.
    ///
    /// Matches C++ `IsPrototypePath(const SdfPath& path)`.
    pub fn is_prototype_path(path: &Path) -> bool {
        path.is_root_prim_path() && path.as_str().starts_with("/__Prototype_")
    }

    /// Return instance prim indexes registered for prototypePath, an empty
    /// vector otherwise.
    ///
    /// Matches C++ `GetInstancePrimIndexesForPrototype(const SdfPath& prototypePath)`.
    pub fn get_instance_prim_indexes_for_prototype(&self, prototype_path: &Path) -> Vec<Path> {
        let prototype_to_prim_indexes = self
            .prototype_to_prim_indexes
            .lock()
            .expect("lock poisoned");
        prototype_to_prim_indexes
            .get(prototype_path)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns the paths of all prototype prims for instance prim
    /// indexes registered with this cache.
    ///
    /// Matches C++ `GetAllPrototypes()`.
    pub fn get_all_prototypes(&self) -> Vec<Path> {
        let prototypes = self.prototype_to_key.lock().expect("lock poisoned");
        prototypes.keys().cloned().collect()
    }

    /// Returns the number of prototype prims assigned to instance
    /// prim indexes registered with this cache.
    ///
    /// Matches C++ `GetNumPrototypes()`.
    pub fn get_num_prototypes(&self) -> usize {
        let prototypes = self.prototype_to_key.lock().expect("lock poisoned");
        prototypes.len()
    }

    /// Return the path of the prototype root prim using the prim index at
    /// primIndexPath as its source prim index, or the empty path if no such
    /// prototype exists.
    ///
    /// Matches C++ `GetPrototypeUsingPrimIndexPath(const SdfPath& primIndexPath)`.
    pub fn get_prototype_using_prim_index_path(&self, prim_index_path: &Path) -> Path {
        let source_map = self
            .source_prim_index_to_prototype
            .lock()
            .expect("lock poisoned");
        source_map
            .get(prim_index_path)
            .cloned()
            .unwrap_or_else(Path::empty)
    }

    /// Return the source prim index path for the given prototype path.
    /// Empty path if no source exists.
    ///
    /// Matches C++ `_instanceCache->prototype_to_source_index` lookup.
    pub fn get_source_index_path_for_prototype(&self, prototype_path: &Path) -> Path {
        let map = self
            .prototype_to_source_index
            .lock()
            .expect("lock poisoned");
        map.get(prototype_path).cloned().unwrap_or_else(Path::empty)
    }

    /// Return the paths of all prims in prototypes using the prim index at
    /// primIndexPath.
    ///
    /// Matches C++ `GetPrimsInPrototypesUsingPrimIndexPath(const SdfPath& primIndexPath)`.
    pub fn get_prims_in_prototypes_using_prim_index_path(
        &self,
        prim_index_path: &Path,
    ) -> Vec<Path> {
        // This function is trickier than you might expect because it has
        // to deal with nested instances. For now, return prototype paths
        // that use this prim index path.
        let mut prototype_paths = Vec::new();
        let source_map = self
            .source_prim_index_to_prototype
            .lock()
            .expect("lock poisoned");

        // Check if this path is a source for any prototype
        if let Some(prototype_path) = source_map.get(prim_index_path) {
            prototype_paths.push(prototype_path.clone());
        }

        // Also check if any prototype uses this path as a descendant
        // (for nested instancing)
        for (source_path, prototype_path) in source_map.iter() {
            if source_path.has_prefix(prim_index_path) && !prototype_paths.contains(prototype_path)
            {
                prototype_paths.push(prototype_path.clone());
            }
        }

        prototype_paths
    }

    /// Return a vector of pair of prototype and respective source prim index
    /// path for all prototypes using the prim index at primIndexPath or as
    /// descendent of primIndexPath.
    ///
    /// Matches C++ `GetPrototypesUsingPrimIndexPathOrDescendents(const SdfPath& primIndexPath)`.
    pub fn get_prototypes_using_prim_index_path_or_descendents(
        &self,
        prim_index_path: &Path,
    ) -> Vec<(Path, Path)> {
        let mut prototype_source_index_pairs = Vec::new();
        let source_map = self
            .source_prim_index_to_prototype
            .lock()
            .expect("lock poisoned");
        let prototype_to_source = self
            .prototype_to_source_index
            .lock()
            .expect("lock poisoned");

        // Find all source paths that are at or under prim_index_path
        for (source_path, prototype_path) in source_map.iter() {
            if source_path.has_prefix(prim_index_path) {
                let source_index_path = prototype_to_source
                    .get(prototype_path)
                    .cloned()
                    .unwrap_or_else(Path::empty);
                prototype_source_index_pairs.push((prototype_path.clone(), source_index_path));
            }
        }

        prototype_source_index_pairs
    }

    /// Return true if a prim in a prototype uses the prim index at
    /// primIndexPath.
    ///
    /// Matches C++ `PrototypeUsesPrimIndexPath(const SdfPath& primIndexPath)`.
    pub fn prototype_uses_prim_index_path(&self, prim_index_path: &Path) -> bool {
        let source_map = self
            .source_prim_index_to_prototype
            .lock()
            .expect("lock poisoned");
        source_map.contains_key(prim_index_path)
    }

    /// Return the path of the prototype prim associated with the instanceable
    /// primIndexPath.
    ///
    /// Matches C++ `GetPrototypeForInstanceablePrimIndexPath(const SdfPath& primIndexPath)`.
    ///
    /// If primIndexPath is not instanceable, or if it has no associated prototype
    /// because it lacks composition arcs, return the empty path.
    pub fn get_prototype_for_instanceable_prim_index_path(&self, prim_index_path: &Path) -> Path {
        let prim_index_map = self.prim_index_to_prototype.lock().expect("lock poisoned");
        prim_index_map
            .get(prim_index_path)
            .cloned()
            .unwrap_or_else(Path::empty)
    }

    /// Returns true if primPath is descendent to an instance.
    ///
    /// Matches C++ `IsPrimPathDescendentToAnInstance(const SdfPath& primPath)`.
    pub fn is_prim_path_descendent_to_an_instance(&self, prim_path: &Path) -> bool {
        // Check if any ancestor of prim_path is an instanceable prim index
        let prim_index_map = self.prim_index_to_prototype.lock().expect("lock poisoned");
        let mut current = prim_path.clone();

        while !current.is_empty() && current != Path::absolute_root() {
            if prim_index_map.contains_key(&current) {
                return true;
            }
            current = current.get_parent_path();
        }

        false
    }

    /// Return the path in the prototype for the given instance path.
    ///
    /// Matches C++ `GetPathInPrototypeForInstancePath(const SdfPath& instancePath)`.
    pub fn get_path_in_prototype_for_instance_path(&self, instance_path: &Path) -> Path {
        // Find the most ancestral instance path
        let most_ancestral_instance = self.get_most_ancestral_instance_path(instance_path);
        if most_ancestral_instance.is_empty() {
            return Path::empty();
        }

        // Get the prototype for this instance
        let prim_index_map = self.prim_index_to_prototype.lock().expect("lock poisoned");
        let Some(prototype_path) = prim_index_map.get(&most_ancestral_instance) else {
            return Path::empty();
        };

        // Translate the instance path to the prototype path
        // Remove the instance prefix and append the remaining relative suffix.
        if !instance_path.has_prefix(&most_ancestral_instance) {
            return prototype_path.clone();
        }

        let instance_text = instance_path.as_str();
        let prefix_text = most_ancestral_instance.as_str();
        let Some(mut suffix_text) = instance_text.strip_prefix(prefix_text) else {
            return prototype_path.clone();
        };

        if suffix_text.is_empty() {
            return prototype_path.clone();
        }

        if let Some(stripped) = suffix_text.strip_prefix('/') {
            suffix_text = stripped;
        }

        let relative_suffix = Path::from_string(suffix_text).unwrap_or_else(Path::empty);
        if relative_suffix.is_empty() {
            prototype_path.clone()
        } else {
            prototype_path
                .append_path(&relative_suffix)
                .unwrap_or_else(|| prototype_path.clone())
        }
    }

    /// Returns the shortest ancestor of primPath that identifies an
    /// instanceable prim. If there is no such ancestor, return the empty path.
    ///
    /// Matches C++ `GetMostAncestralInstancePath(const SdfPath& primPath)`.
    pub fn get_most_ancestral_instance_path(&self, prim_path: &Path) -> Path {
        let prim_index_map = self.prim_index_to_prototype.lock().expect("lock poisoned");
        let mut current = prim_path.clone();
        let mut most_ancestral = Path::empty();

        while !current.is_empty() && current != Path::absolute_root() {
            if prim_index_map.contains_key(&current) {
                most_ancestral = current.clone();
            }
            current = current.get_parent_path();
        }

        most_ancestral
    }

    /// Return the instance path for the given prototype path.
    ///
    /// Matches C++ `GetInstancePathForPrototypePath(const SdfPath& prototypePath)`.
    pub fn get_instance_path_for_prototype_path(&self, prototype_path: &Path) -> Vec<Path> {
        // Reverse lookup: find all prim indexes that map to this prototype
        let prim_index_map = self.prim_index_to_prototype.lock().expect("lock poisoned");
        prim_index_map
            .iter()
            .filter_map(|(index_path, proto_path)| {
                if proto_path == prototype_path {
                    Some(index_path.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Creates or updates a prototype for instances with the given key.
    ///
    /// Matches C++ `_CreateOrUpdatePrototypeForInstances`.
    fn create_or_update_prototype_for_instances(
        &self,
        key: &InstanceKey,
        prim_index_paths: &mut Vec<Path>,
        changes: &mut InstanceChanges,
        prototype_to_old_source_index_path: &HashMap<Path, Path>,
    ) {
        let mut key_to_prototype = self.key_to_prototype.lock().expect("lock poisoned");
        let mut prototype_to_key = self.prototype_to_key.lock().expect("lock poisoned");
        let mut source_map = self
            .source_prim_index_to_prototype
            .lock()
            .expect("lock poisoned");
        let mut prototype_to_source = self
            .prototype_to_source_index
            .lock()
            .expect("lock poisoned");
        let mut prototype_to_prim_indexes = self
            .prototype_to_prim_indexes
            .lock()
            .expect("lock poisoned");
        let mut prim_index_to_prototype =
            self.prim_index_to_prototype.lock().expect("lock poisoned");

        // Try to insert or get existing prototype
        let (prototype_path, created_new_prototype) = {
            let entry = key_to_prototype.entry(key.clone());
            match entry {
                std::collections::hash_map::Entry::Occupied(entry) => (entry.get().clone(), false),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let new_prototype_path = self.get_next_prototype_path(key);
                    entry.insert(new_prototype_path.clone());
                    (new_prototype_path, true)
                }
            }
        };

        if created_new_prototype {
            // If this is a new prototype prim, the first instanceable prim
            // index that was registered must be selected as the source
            // index because the consumer was told that index required
            // a new prototype via RegisterInstancePrimIndex.
            prototype_to_key.insert(prototype_path.clone(), key.clone());

            let source_prim_index_path = prim_index_paths[0].clone();
            source_map.insert(source_prim_index_path.clone(), prototype_path.clone());
            prototype_to_source.insert(prototype_path.clone(), source_prim_index_path.clone());

            changes.new_prototype_prims.push(prototype_path.clone());
            changes
                .new_prototype_prim_indexes
                .push(source_prim_index_path);
        } else {
            // Otherwise, if a prototype prim for this instance already exists
            // but no source prim index has been assigned, do so here.
            let assign_new_prim_index_for_prototype =
                !prototype_to_source.contains_key(&prototype_path);
            if assign_new_prim_index_for_prototype {
                let source_prim_index_path = prim_index_paths[0].clone();
                source_map.insert(source_prim_index_path.clone(), prototype_path.clone());
                prototype_to_source.insert(prototype_path.clone(), source_prim_index_path.clone());

                changes.changed_prototype_prims.push(prototype_path.clone());
                changes
                    .changed_prototype_prim_indexes
                    .push(source_prim_index_path.clone());

                if let Some(_old_source_path) =
                    prototype_to_old_source_index_path.get(&prototype_path)
                {
                    // Log change if needed
                }
            }
        }

        // Assign the newly-registered prim indexes to their prototype.
        for prim_index_path in prim_index_paths.iter() {
            prim_index_to_prototype.insert(prim_index_path.clone(), prototype_path.clone());
        }

        // Merge prim indexes into prototype's list
        let prim_indexes_for_prototype = prototype_to_prim_indexes
            .entry(prototype_path.clone())
            .or_default();

        if prim_indexes_for_prototype.is_empty() {
            // Move all prim_index_paths into the list
            *prim_indexes_for_prototype = std::mem::take(prim_index_paths);
        } else {
            // Append and deduplicate
            let mut new_paths = std::mem::take(prim_index_paths);
            prim_indexes_for_prototype.append(&mut new_paths);
            prim_indexes_for_prototype.sort();
            prim_indexes_for_prototype.dedup();
        }
    }

    /// Removes instances from the cache.
    ///
    /// Matches C++ `_RemoveInstances`.
    fn remove_instances(
        &self,
        key: &InstanceKey,
        prim_index_paths: &[Path],
        changes: &mut InstanceChanges,
        prototype_to_old_source_index_path: &mut HashMap<Path, Path>,
    ) {
        if prim_index_paths.is_empty() {
            return;
        }

        let key_to_prototype = self.key_to_prototype.lock().expect("lock poisoned");
        let Some(prototype_path) = key_to_prototype.get(key).cloned() else {
            return;
        };
        drop(key_to_prototype);

        let mut prototype_to_prim_indexes = self
            .prototype_to_prim_indexes
            .lock()
            .expect("lock poisoned");
        let mut prim_index_to_prototype =
            self.prim_index_to_prototype.lock().expect("lock poisoned");
        let mut source_map = self
            .source_prim_index_to_prototype
            .lock()
            .expect("lock poisoned");
        let mut prototype_to_source = self
            .prototype_to_source_index
            .lock()
            .expect("lock poisoned");

        let prim_indexes_for_prototype = prototype_to_prim_indexes
            .get_mut(&prototype_path)
            .expect("Prototype should exist");

        let mut removed_prototype_prim_index_path = Path::empty();

        // Remove the prim indexes from the prim index <-> prototype bidirectional mapping.
        for path in prim_index_paths.iter() {
            if let Some(pos) = prim_indexes_for_prototype.iter().position(|p| p == path) {
                prim_indexes_for_prototype.remove(pos);
                prim_index_to_prototype.remove(path);
            }

            // Check if this was the source prim index
            if source_map.remove(path).is_some() {
                prototype_to_source.remove(&prototype_path);
                removed_prototype_prim_index_path = path.clone();
            }
        }

        // If the source prim index for this prototype is no longer available
        // but we have other instance prim indexes we can use instead, select
        // one of those to serve as the new source.
        if !removed_prototype_prim_index_path.is_empty() {
            if !prim_indexes_for_prototype.is_empty() {
                let new_source_index_path = prim_indexes_for_prototype[0].clone();
                source_map.insert(new_source_index_path.clone(), prototype_path.clone());
                prototype_to_source.insert(prototype_path.clone(), new_source_index_path.clone());

                changes.changed_prototype_prims.push(prototype_path.clone());
                changes
                    .changed_prototype_prim_indexes
                    .push(new_source_index_path);
            } else {
                // Save old source path for later use
                prototype_to_old_source_index_path
                    .insert(prototype_path.clone(), removed_prototype_prim_index_path);
            }
        }
    }

    /// Removes a prototype if it has no instances.
    ///
    /// Matches C++ `_RemovePrototypeIfNoInstances`.
    fn remove_prototype_if_no_instances(&self, key: &InstanceKey, changes: &mut InstanceChanges) {
        let key_to_prototype = self.key_to_prototype.lock().expect("lock poisoned");
        let Some(prototype_path) = key_to_prototype.get(key).cloned() else {
            return;
        };
        drop(key_to_prototype);

        let prototype_has_no_instances = self
            .prototype_to_prim_indexes
            .lock()
            .expect("lock poisoned")
            .get(&prototype_path)
            .is_some_and(|prim_indexes| prim_indexes.is_empty());

        if prototype_has_no_instances {
            // This prototype has no more instances associated with it, so it can be released.
            changes.dead_prototype_prims.push(prototype_path.clone());

            let mut key_to_prototype = self.key_to_prototype.lock().expect("lock poisoned");
            let mut prototype_to_key = self.prototype_to_key.lock().expect("lock poisoned");
            let mut prototype_to_source = self
                .prototype_to_source_index
                .lock()
                .expect("lock poisoned");
            let mut source_map = self
                .source_prim_index_to_prototype
                .lock()
                .expect("lock poisoned");
            let mut prototype_to_prim_indexes = self
                .prototype_to_prim_indexes
                .lock()
                .expect("lock poisoned");

            key_to_prototype.remove(key);
            prototype_to_key.remove(&prototype_path);
            if let Some(source_path) = prototype_to_source.remove(&prototype_path) {
                source_map.remove(&source_path);
            }
            prototype_to_prim_indexes.remove(&prototype_path);
        }
    }

    /// Gets the next prototype path for the given key.
    ///
    /// Matches C++ `_GetNextPrototypePath`.
    fn get_next_prototype_path(&self, _key: &InstanceKey) -> Path {
        let mut index = self.last_prototype_index.lock().expect("lock poisoned");
        *index += 1;
        let name = format!("__Prototype_{}", *index);
        Path::absolute_root()
            .append_child(&name)
            .unwrap_or_else(|| {
                // Fallback if append_child fails
                Path::from_string(&format!("/{}", name)).unwrap_or_else(Path::empty)
            })
    }
}

impl Default for InstanceCache {
    fn default() -> Self {
        Self::new()
    }
}
