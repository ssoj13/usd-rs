
//! Caching scene index - caches prim data and child paths.
//!
//! G22: Two-tier cache (primary BTreeMap + recent concurrent HashMap).
//! G23: HD_CACHING_SCENE_INDEX_USE_CONVERVATIVE_EVICTION env setting (C++ typo preserved).

use super::base::{HdSceneIndexBase, HdSceneIndexHandle, SdfPathVector, TfTokenVector};
use super::filtering::{FilteringObserverTarget, HdSingleInputFilteringSceneIndexBase};
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use super::prim::HdSceneIndexPrim;
use parking_lot::RwLock as ParkingRwLock;
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// G23: Check env var for conservative eviction.
///
/// When true, eviction only removes the specific prim, not the subtree.
/// Port of C++ HD_CACHING_SCENE_INDEX_USE_CONVERVATIVE_EVICTION (C++ typo preserved).
fn conservative_eviction_enabled() -> bool {
    // NOTE: C++ env var has typo "CONVERVATIVE" — we match it exactly for compat
    std::env::var("HD_CACHING_SCENE_INDEX_USE_CONVERVATIVE_EVICTION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// A scene index that caches prim data and child prim paths.
///
/// This is a pass-through scene index that caches queries to its input.
/// Useful for expensive input scene indices where queries are repeated.
///
/// G22: Two-tier cache: primary BTreeMap for ordered traversal + recent
/// concurrent HashMap for fast repeated lookups.
/// G23: Supports HD_CACHING_SCENE_INDEX_USE_CONVERVATIVE_EVICTION (C++ typo preserved).
pub struct HdCachingSceneIndex {
    /// Filtering base
    filtering_base: HdSingleInputFilteringSceneIndexBase,
    /// G22: Primary prim cache (sorted for subtree ops).
    /// None = cached as absent (prim doesn't exist), missing = not yet cached.
    prim_cache: RwLock<BTreeMap<SdfPath, Option<HdSceneIndexPrim>>>,
    /// G22: Recent prim cache (fast concurrent reads).
    /// None = cached as absent, missing = not yet cached.
    recent_prim_cache: ParkingRwLock<HashMap<SdfPath, Option<HdSceneIndexPrim>>>,
    /// G22: Primary child paths cache (sorted)
    child_paths_cache: RwLock<BTreeMap<SdfPath, SdfPathVector>>,
    /// G22: Recent child paths cache (fast concurrent reads)
    recent_child_paths_cache: ParkingRwLock<HashMap<SdfPath, SdfPathVector>>,
    /// G23: Conservative eviction setting (cached at construction)
    conservative_eviction: bool,
}

impl HdCachingSceneIndex {
    /// Create a new caching scene index.
    ///
    /// C++ parity: constructor calls `_inputSceneIndex->AddObserver(this)`.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        let input_clone = input_scene.clone();
        let result = Arc::new(RwLock::new(Self {
            filtering_base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            prim_cache: RwLock::new(BTreeMap::new()),
            recent_prim_cache: ParkingRwLock::new(HashMap::new()),
            child_paths_cache: RwLock::new(BTreeMap::new()),
            recent_child_paths_cache: ParkingRwLock::new(HashMap::new()),
            conservative_eviction: conservative_eviction_enabled(),
        }));
        if let Some(input) = input_clone {
            super::filtering::wire_filter_to_input(&result, &input);
        }
        result
    }

    /// Clear the entire cache.
    pub fn clear_cache(&self) {
        {
            let mut cache = self.prim_cache.write();
            cache.clear();
        }
        self.recent_prim_cache.write().clear();
        {
            let mut cache = self.child_paths_cache.write();
            cache.clear();
        }
        self.recent_child_paths_cache.write().clear();
    }

    /// Invalidate cache entries for a prim and its descendants.
    ///
    /// G23: When conservative_eviction is enabled, only invalidate the prim
    /// itself, not its descendants.
    fn invalidate_prim_subtree(&self, path: &SdfPath) {
        let mut prim_cache = self.prim_cache.write();
        let mut child_cache = self.child_paths_cache.write();

        if self.conservative_eviction {
            // G23: Conservative - only remove the specific prim
            prim_cache.remove(path);
            child_cache.remove(path);
            // Also invalidate parent's child list
            if !path.is_absolute_root_path() {
                let parent = path.get_parent_path();
                child_cache.remove(&parent);
            }
        } else {
            // Full subtree invalidation using BTreeMap range (G22/G8 pattern)
            let to_remove: Vec<SdfPath> = prim_cache
                .range(path.clone()..)
                .take_while(|(k, _)| k.has_prefix(path))
                .map(|(k, _)| k.clone())
                .collect();
            for key in &to_remove {
                prim_cache.remove(key);
                child_cache.remove(key);
            }

            // Invalidate parent's child list
            if !path.is_absolute_root_path() {
                let parent = path.get_parent_path();
                child_cache.remove(&parent);
            }
        }

        // G22: Clear recent caches
        self.recent_prim_cache
            .write()
            .retain(|k, _| !k.has_prefix(path));
        self.recent_child_paths_cache
            .write()
            .retain(|k, _| !k.has_prefix(path));
    }
}

impl HdSceneIndexBase for HdCachingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        // G22: Check recent cache first (fast path)
        {
            let recent = self.recent_prim_cache.read();
            if let Some(cached) = recent.get(prim_path) {
                // Some(prim) = cached present, None = cached as absent
                return cached.clone().unwrap_or_else(HdSceneIndexPrim::empty);
            }
        }

        // Check primary cache
        {
            let cache = self.prim_cache.read();
            if let Some(cached) = cache.get(prim_path) {
                // Promote to recent cache
                self.recent_prim_cache
                    .write()
                    .insert(prim_path.clone(), cached.clone());
                return cached.clone().unwrap_or_else(HdSceneIndexPrim::empty);
            }
        }

        // Query input scene and cache result
        if let Some(input) = self.filtering_base.get_input_scene() {
            {
                let input_lock = input.read();
                let prim = input_lock.get_prim(prim_path);

                // Cache: if prim is defined store Some, else store None (cached-null)
                let cache_val = if prim.is_defined() {
                    Some(prim.clone())
                } else {
                    None
                };

                {
                    let mut cache = self.prim_cache.write();
                    cache.insert(prim_path.clone(), cache_val.clone());
                }
                self.recent_prim_cache
                    .write()
                    .insert(prim_path.clone(), cache_val);

                return prim;
            }
        }

        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        // G22: Check recent cache first
        {
            let recent = self.recent_child_paths_cache.read();
            if let Some(cached) = recent.get(prim_path) {
                return cached.clone();
            }
        }

        // Check primary cache
        {
            let cache = self.child_paths_cache.read();
            if let Some(cached) = cache.get(prim_path) {
                self.recent_child_paths_cache
                    .write()
                    .insert(prim_path.clone(), cached.clone());
                return cached.clone();
            }
        }

        // Query input scene and cache result
        if let Some(input) = self.filtering_base.get_input_scene() {
            {
                let input_lock = input.read();
                let children = input_lock.get_child_prim_paths(prim_path);

                {
                    let mut cache = self.child_paths_cache.write();
                    cache.insert(prim_path.clone(), children.clone());
                }
                self.recent_child_paths_cache
                    .write()
                    .insert(prim_path.clone(), children.clone());

                return children;
            }
        }

        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.filtering_base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.filtering_base.base().remove_observer(observer);
    }

    fn set_display_name(&mut self, name: String) {
        self.filtering_base.base_mut().set_display_name(name);
    }

    fn add_tag(&mut self, tag: TfToken) {
        self.filtering_base.base_mut().add_tag(tag);
    }

    fn remove_tag(&mut self, tag: &TfToken) {
        self.filtering_base.base_mut().remove_tag(tag);
    }

    fn has_tag(&self, tag: &TfToken) -> bool {
        self.filtering_base.base().has_tag(tag)
    }

    fn get_tags(&self) -> TfTokenVector {
        self.filtering_base.base().get_tags()
    }

    fn get_display_name(&self) -> String {
        let name = self.filtering_base.base().get_display_name();
        if name.is_empty() {
            "HdCachingSceneIndex".to_string()
        } else {
            name.to_string()
        }
    }

    /// G2: SystemMessage recursion through input scene.
    fn get_input_scenes_for_system_message(&self) -> Vec<super::base::HdSceneIndexHandle> {
        self.filtering_base
            .get_input_scene()
            .cloned()
            .into_iter()
            .collect()
    }
}

impl FilteringObserverTarget for HdCachingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        for entry in entries {
            self.invalidate_prim_subtree(&entry.prim_path);
        }
        self.filtering_base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        for entry in entries {
            self.invalidate_prim_subtree(&entry.prim_path);
        }
        self.filtering_base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        // Invalidate cache for dirtied prims
        {
            let mut cache = self.prim_cache.write();
            for entry in entries {
                cache.remove(&entry.prim_path);
            }
        }
        {
            let mut recent = self.recent_prim_cache.write();
            for entry in entries {
                recent.remove(&entry.prim_path);
            }
        }
        self.filtering_base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        for entry in entries {
            self.invalidate_prim_subtree(&entry.old_prim_path);
            self.invalidate_prim_subtree(&entry.new_prim_path);
        }
        self.filtering_base.forward_prims_renamed(self, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caching_scene_creation() {
        let scene = HdCachingSceneIndex::new(None);
        let scene_lock = scene.read();

        let prim = scene_lock.get_prim(&SdfPath::absolute_root());
        assert!(!prim.is_defined());
    }

    #[test]
    fn test_clear_cache() {
        let scene = HdCachingSceneIndex::new(None);
        let scene_lock = scene.read();
        scene_lock.clear_cache();
    }
}
