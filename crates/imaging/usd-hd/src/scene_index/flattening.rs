//! Flattening scene index - flattens hierarchy for inherited state.
//!
//! Port of pxr/imaging/hd/flatteningSceneIndex.h/.cpp.
//!
//! Observes an input scene index and produces a scene where inherited state
//! (xform, visibility, purpose, material bindings, primvars) is resolved
//! at each prim. This enables render delegates that require all relevant
//! information at leaf prims without walking the hierarchy.

use super::base::{
    HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle, SdfPathVector,
    TfTokenVector, si_ref,
};
use super::filtering::{
    FilteringObserverTarget, FilteringSceneIndexObserver, HdSingleInputFilteringSceneIndexBase,
};
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use super::prim::HdSceneIndexPrim;
use crate::data_source::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet,
};
use crate::flattened_data_source_provider::{
    HdFlattenedDataSourceProviderContext, HdFlattenedDataSourceProviderHandle,
    HdFlattenedDataSourceProviderVector,
};
use crate::flo_debug::flo_debug_enabled;
use parking_lot::RwLock as ParkingRwLock;
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

static DEBUG_XFORM_GET_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_XFORM_GET_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_XFORM_GET_COMPUTES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_XFORM_GET_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_XFORM_GET_COMPUTE_NS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugFlatteningXformGetStats {
    pub calls: usize,
    pub cache_hits: usize,
    pub computes: usize,
    pub total_ns: u64,
    pub compute_ns: u64,
}

pub fn reset_debug_flattening_xform_get_stats() {
    DEBUG_XFORM_GET_CALLS.store(0, Ordering::Relaxed);
    DEBUG_XFORM_GET_CACHE_HITS.store(0, Ordering::Relaxed);
    DEBUG_XFORM_GET_COMPUTES.store(0, Ordering::Relaxed);
    DEBUG_XFORM_GET_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_XFORM_GET_COMPUTE_NS.store(0, Ordering::Relaxed);
}

pub fn read_debug_flattening_xform_get_stats() -> DebugFlatteningXformGetStats {
    DebugFlatteningXformGetStats {
        calls: DEBUG_XFORM_GET_CALLS.load(Ordering::Relaxed),
        cache_hits: DEBUG_XFORM_GET_CACHE_HITS.load(Ordering::Relaxed),
        computes: DEBUG_XFORM_GET_COMPUTES.load(Ordering::Relaxed),
        total_ns: DEBUG_XFORM_GET_TOTAL_NS.load(Ordering::Relaxed),
        compute_ns: DEBUG_XFORM_GET_COMPUTE_NS.load(Ordering::Relaxed),
    }
}

// ---------------------------------------------------------------------------
// PrimLevelWrappingDataSource
// ---------------------------------------------------------------------------

/// Shared mutable state for [`PrimLevelWrappingDataSource`].
///
/// This mirrors the C++ `_PrimLevelWrappingDataSource` handle semantics:
/// cloned/type-erased handles must keep pointing at the same caches so that
/// flattening invalidation propagates through `clone_box()` and
/// `cast_to_container()` call sites.
struct PrimLevelWrappingDataSourceInner {
    /// Weak ref to the flattening scene index (for parent prim lookups).
    scene_weak: Weak<RwLock<HdFlatteningSceneIndex>>,
    /// Path of this prim.
    prim_path: SdfPath,
    /// Type of this prim.
    prim_type: TfToken,
    /// Cached input data source (None = not yet fetched, Some(None) = null).
    input_ds_cache: RwLock<Option<Option<HdContainerDataSourceHandle>>>,
    /// Initial input data source from construction.
    initial_input_ds: Option<HdContainerDataSourceHandle>,
    /// Cached flattened data sources, parallel to `ds_names`.
    /// None = not yet computed, Some(None) = computed as null, Some(Some(ds)) = computed.
    computed_ds: Vec<RwLock<Option<Option<HdContainerDataSourceHandle>>>>,
    /// Names of data sources to flatten (shared).
    ds_names: Arc<Vec<TfToken>>,
    /// Providers for each flattened data source (shared).
    providers: Arc<HdFlattenedDataSourceProviderVector>,
}

/// Wraps input prim data source to intercept flattened data source lookups.
///
/// For each registered flattened name (e.g. "xform", "visibility"), this
/// data source lazily computes and caches the flattened result using the
/// corresponding provider. Non-flattened names are delegated to the input.
///
/// Port of C++ `HdFlatteningSceneIndex_Impl::_PrimLevelWrappingDataSource`.
#[derive(Clone)]
struct PrimLevelWrappingDataSource {
    /// Shared wrapper state so erased/cloned handles keep observing the same
    /// invalidation and cache lifetimes.
    inner: Arc<PrimLevelWrappingDataSourceInner>,
}

impl PrimLevelWrappingDataSource {
    /// Create a new wrapping data source.
    fn new(
        scene_weak: Weak<RwLock<HdFlatteningSceneIndex>>,
        prim_path: SdfPath,
        input_prim: &HdSceneIndexPrim,
        ds_names: Arc<Vec<TfToken>>,
        providers: Arc<HdFlattenedDataSourceProviderVector>,
    ) -> Arc<Self> {
        let num_providers = ds_names.len();
        let mut computed = Vec::with_capacity(num_providers);
        for _ in 0..num_providers {
            computed.push(RwLock::new(None));
        }
        Arc::new(Self {
            inner: Arc::new(PrimLevelWrappingDataSourceInner {
                scene_weak,
                prim_path,
                prim_type: input_prim.prim_type.clone(),
                input_ds_cache: RwLock::new(Some(input_prim.data_source.clone())),
                initial_input_ds: input_prim.data_source.clone(),
                computed_ds: computed,
                ds_names,
                providers,
            }),
        })
    }

    /// Get the input prim data source, lazily re-fetching if invalidated.
    fn get_input_ds(&self) -> Option<HdContainerDataSourceHandle> {
        // Check cache
        {
            let cache = self.inner.input_ds_cache.read();
            if let Some(ref cached) = *cache {
                return cached.clone();
            }
        }

        // Re-fetch from input scene
        if let Some(scene_arc) = self.inner.scene_weak.upgrade() {
            let scene = super::base::rwlock_data_ref(scene_arc.as_ref());
            if let Some(input) = scene.filtering_base.get_input_scene() {
                let input_ref = si_ref(input);
                let prim = input_ref.get_prim(&self.inner.prim_path);
                let ds = prim.data_source;
                {
                    let mut cache = self.inner.input_ds_cache.write();
                    *cache = Some(ds.clone());
                }
                return ds;
            }
        }

        self.inner.initial_input_ds.clone()
    }

    /// Invalidate cached data sources based on dirty locators.
    ///
    /// Port of C++ `_PrimLevelWrappingDataSource::PrimDirtied`.
    /// `relative_dirty_locators` is parallel to data_source_names.
    fn prim_dirtied(&self, relative_dirty_locators: &[HdDataSourceLocatorSet]) -> bool {
        let mut any_dirtied = false;

        for (i, dirty_locs) in relative_dirty_locators.iter().enumerate() {
            if dirty_locs.is_empty() {
                continue;
            }

            // Check if we have a cached data source
            let has_cached = self.inner.computed_ds[i].read().is_some();

            if !has_cached {
                continue;
            }

            let mut cache = self.inner.computed_ds[i].write();
            if let Some(Some(cached_ds)) = cache.as_ref() {
                if !dirty_locs.contains(&HdDataSourceLocator::empty()) {
                    if let Some(invalidatable_ds) = cached_ds.as_invalidatable_container() {
                        if invalidatable_ds.invalidate(dirty_locs) {
                            any_dirtied = true;
                            continue;
                        }
                    }
                }

                *cache = None;
                any_dirtied = true;
            } else {
                // `Some(None)` means we've already cached the absence of a data source,
                // which still counts as a cached flattening result that must be forgotten.
                if cache.is_some() {
                    *cache = None;
                    any_dirtied = true;
                } else {
                    continue;
                }
            }
        }

        any_dirtied
    }

    /// Invalidate the input prim container data source.
    fn prim_container_dirtied(&self) {
        let mut cache = self.inner.input_ds_cache.write();
        *cache = None;
    }
}

impl std::fmt::Debug for PrimLevelWrappingDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimLevelWrappingDataSource")
            .field("prim_path", &self.inner.prim_path)
            .field("prim_type", &self.inner.prim_type)
            .finish()
    }
}

impl HdDataSourceBase for PrimLevelWrappingDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone()) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimLevelWrappingDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        let input_ds = self.get_input_ds();

        if let Some(ref ds) = input_ds {
            let mut result = ds.get_names();
            // Insert flattened names that aren't already present
            insert_unique_names(&self.inner.ds_names, &mut result);
            result
        } else {
            // No input - just return the flattened names
            self.inner.ds_names.as_ref().clone()
        }
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        // Check if this is a flattened data source name
        for (i, ds_name) in self.inner.ds_names.iter().enumerate() {
            if name != ds_name {
                continue;
            }

            let debug_xform = flo_debug_enabled() && name.as_str() == "xform";
            let started = debug_xform.then(std::time::Instant::now);
            if debug_xform {
                DEBUG_XFORM_GET_CALLS.fetch_add(1, Ordering::Relaxed);
            }

            // Check cache
            {
                let cache = self.inner.computed_ds[i].read();
                if let Some(ref cached) = *cache {
                    if debug_xform {
                        DEBUG_XFORM_GET_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                        if let Some(started) = started {
                            DEBUG_XFORM_GET_TOTAL_NS
                                .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                        }
                    }
                    return cached.clone().map(|c| c as HdDataSourceBaseHandle);
                }
            }

            // Compute the flattened data source via provider.
            // Use data_ptr() to avoid recursive read-lock deadlock —
            // provider calls get_prim on the same FlatteningSceneIndex.
            let scene_arc = self.inner.scene_weak.upgrade()?;
            let scene_lock = super::base::rwlock_data_ref(scene_arc.as_ref());

            let input_ds = self.get_input_ds();
            let input_prim = HdSceneIndexPrim {
                prim_type: self.inner.prim_type.clone(),
                data_source: input_ds,
            };

            let ctx = HdFlattenedDataSourceProviderContext {
                flattening_scene_index: &*scene_lock,
                flattening_scene_index_weak: self.inner.scene_weak.clone(),
                prim_path: &self.inner.prim_path,
                name: ds_name,
                input_prim: &input_prim,
            };

            let compute_started = debug_xform.then(std::time::Instant::now);
            let result = self.inner.providers[i].get_flattened_data_source(&ctx);
            if debug_xform {
                DEBUG_XFORM_GET_COMPUTES.fetch_add(1, Ordering::Relaxed);
                if let Some(compute_started) = compute_started {
                    DEBUG_XFORM_GET_COMPUTE_NS.fetch_add(
                        compute_started.elapsed().as_nanos() as u64,
                        Ordering::Relaxed,
                    );
                }
            }

            // Cache
            {
                let mut cache = self.inner.computed_ds[i].write();
                *cache = Some(result.clone());
            }

            if debug_xform {
                if let Some(started) = started {
                    DEBUG_XFORM_GET_TOTAL_NS
                        .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
            }
            return result.map(|c| c as HdDataSourceBaseHandle);
        }

        // Not a flattened name - delegate to input data source
        let input_ds = self.get_input_ds()?;
        input_ds.get(name)
    }
}

/// Insert tokens from `src` into `result` without duplicates.
fn insert_unique_names(src: &[TfToken], result: &mut Vec<TfToken>) {
    // Optimized for small src vectors (typically < 8 flattened names)
    for name in src {
        if !result.iter().any(|r| r == name) {
            result.push(name.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// HdFlatteningSceneIndex
// ---------------------------------------------------------------------------

/// A scene index that produces a scene with inherited state at leaf prims.
///
/// Observes an input scene index and resolves inherited state (transforms,
/// visibility, purpose, material bindings, primvars) so that downstream
/// render delegates see fully composed values at each prim.
///
/// Port of C++ `HdFlatteningSceneIndex`.
pub struct HdFlatteningSceneIndex {
    /// Filtering base
    filtering_base: HdSingleInputFilteringSceneIndexBase,
    /// Self-weak reference for PrimLevelWrappingDataSource to call get_prim
    self_weak: Option<Weak<RwLock<HdFlatteningSceneIndex>>>,
    /// Names of data sources to flatten, parallel to providers
    data_source_names: Arc<Vec<TfToken>>,
    /// Providers for computing flattened data, parallel to names
    data_source_providers: Arc<HdFlattenedDataSourceProviderVector>,
    /// All flattened locators (one per name) for quick dirty check
    data_source_locator_set: HdDataSourceLocatorSet,
    /// Universal sets per name for dirtying ancestors of resynced prims
    relative_data_source_locators: Vec<HdDataSourceLocatorSet>,
    /// G6: Primary prim table - sorted by path for efficient subtree iteration (G8).
    /// Port of C++ `SdfPathTable<HdSceneIndexPrim> _prims`.
    prims: RwLock<BTreeMap<SdfPath, HdSceneIndexPrim>>,
    /// G6: Recent prim cache - lock-free secondary tier for concurrent reads.
    /// Port of C++ `tbb::concurrent_hash_map _recentPrims`.
    recent_prims: ParkingRwLock<HashMap<SdfPath, HdSceneIndexPrim>>,
    /// Observer registered on the input scene — kept alive so its weak ref doesn't dangle.
    input_observer: RwLock<Option<HdSceneIndexObserverHandle>>,
}

impl HdFlatteningSceneIndex {
    /// Create a new flattening scene index.
    ///
    /// `input_scene` is the scene to flatten.
    /// `input_args` is a container mapping TfToken names to
    /// `HdFlattenedDataSourceProviderHandle` values, specifying which
    /// data sources to flatten and how.
    pub fn new(
        input_scene: Option<HdSceneIndexHandle>,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let mut names = Vec::new();
        let mut providers: HdFlattenedDataSourceProviderVector = Vec::new();
        let mut locator_set = HdDataSourceLocatorSet::new();
        let mut relative_locators = Vec::new();

        // Parse input_args to extract provider configuration.
        // Each name in the container maps to a provider.
        if let Some(ref args) = input_args {
            for name in args.get_names() {
                if let Some(child) = args.get(&name) {
                    if let Some(provider) = extract_provider_from_ds(&child) {
                        names.push(name.clone());
                        providers.push(provider);
                        locator_set.insert(HdDataSourceLocator::from_token(name));
                        relative_locators.push(HdDataSourceLocatorSet::universal());
                    }
                }
            }
        }

        let arc = Arc::new(RwLock::new(Self {
            filtering_base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            self_weak: None,
            data_source_names: Arc::new(names),
            data_source_providers: Arc::new(providers),
            data_source_locator_set: locator_set,
            relative_data_source_locators: relative_locators,
            prims: RwLock::new(BTreeMap::new()),
            recent_prims: ParkingRwLock::new(HashMap::new()),
            input_observer: RwLock::new(None),
        }));

        // Store self-weak reference and register as observer on the input scene.
        {
            let weak: std::sync::Weak<RwLock<dyn FilteringObserverTarget>> =
                Arc::downgrade(&(arc.clone() as Arc<RwLock<dyn FilteringObserverTarget>>));
            let observer_handle: HdSceneIndexObserverHandle =
                Arc::new(FilteringSceneIndexObserver::new(weak));
            let mut write = arc.write();
            write.self_weak = Some(Arc::downgrade(&arc));
            if let Some(input) = write.filtering_base.get_input_scene() {
                {
                    let input_lock = input.write();
                    input_lock.add_observer(observer_handle.clone());
                }
            }
            *write.input_observer.write() = Some(observer_handle);
        }

        arc
    }

    /// Get the names of data sources being flattened.
    pub fn get_flattened_data_source_names(&self) -> &[TfToken] {
        &self.data_source_names
    }

    /// Get the providers for flattened data sources.
    pub fn get_flattened_data_source_providers(&self) -> &[HdFlattenedDataSourceProviderHandle] {
        &self.data_source_providers
    }

    /// Merge the fast recent-prim cache back into the primary path table before
    /// any subtree walk that must see the full cached hierarchy.
    ///
    /// OpenUSD calls `_ConsolidateRecentPrims()` before every add/remove/dirty
    /// batch. Without that step, `_DirtyHierarchy()` can iterate a stale view of
    /// the cached subtree and incorrectly conclude that descendants were never
    /// cached, which suppresses downstream dirties.
    fn consolidate_recent_prims(&self) {
        let mut recent = self.recent_prims.write();
        if recent.is_empty() {
            return;
        }

        let mut prims = self.prims.write();
        for (path, recent_prim) in recent.drain() {
            if let Some(existing) = prims.get_mut(&path) {
                *existing = recent_prim;
            } else {
                prims.insert(path, recent_prim);
            }
        }
    }

    /// Dirty the hierarchy below `prim_path` for the given locators.
    ///
    /// G8: Uses BTreeMap range for skip-to-next-subtree, avoiding O(N) scan.
    /// Walks cached descendants and invalidates their flattened data sources.
    /// Descendants that were invalidated get added to `additional_dirty`.
    fn dirty_hierarchy(
        &self,
        prim_path: &SdfPath,
        relative_dirty_locators: &[HdDataSourceLocatorSet],
        dirty_locators: &HdDataSourceLocatorSet,
        additional_dirty: &mut Vec<DirtiedPrimEntry>,
    ) {
        let prims = self.prims.read();
        let mut iter = prims.range(prim_path.clone()..).peekable();

        // G8: BTreeMap range scan - start from prim_path, stop when prefix no longer matches.
        // Match OpenUSD's `_DirtyHierarchy()` optimization: if a prim does not
        // invalidate any cached flattened data, skip its entire subtree because
        // deeper descendants cannot depend on it for the flattened result.
        while let Some((path, prim)) = iter.next() {
            if !path.has_prefix(prim_path) {
                break; // Sorted order: no more descendants possible
            }

            let mut can_skip_subtree = false;
            if let Some(ref ds) = prim.data_source {
                if let Some(wrapping_ds) = ds.as_any().downcast_ref::<PrimLevelWrappingDataSource>()
                {
                    let invalidated = wrapping_ds.prim_dirtied(relative_dirty_locators);
                    if invalidated && path != prim_path {
                        additional_dirty
                            .push(DirtiedPrimEntry::new(path.clone(), dirty_locators.clone()));
                    } else if !invalidated {
                        can_skip_subtree = true;
                    }
                }
            }

            if can_skip_subtree {
                let subtree_root = path.clone();
                while let Some((next_path, _)) = iter.peek() {
                    if next_path.has_prefix(&subtree_root) {
                        iter.next();
                    } else {
                        break;
                    }
                }
            }
        }
    }

    /// Process a single dirtied prim entry.
    ///
    /// Computes relative dirty locators per provider, expands them via
    /// ComputeDirtyLocatorsForDescendants, and dirties the hierarchy.
    fn process_prim_dirtied(
        &self,
        entry: &DirtiedPrimEntry,
        additional_dirty: &mut Vec<DirtiedPrimEntry>,
    ) {
        let num_names = self.data_source_names.len();
        let mut relative_dirty_locators: Vec<HdDataSourceLocatorSet> =
            vec![HdDataSourceLocatorSet::new(); num_names];
        let mut dirty_locators = HdDataSourceLocatorSet::new();

        for i in 0..num_names {
            let locator = HdDataSourceLocator::from_token(self.data_source_names[i].clone());

            if !entry.dirty_locators.intersects_locator(&locator) {
                continue;
            }

            if entry.dirty_locators.contains(&locator) {
                // Nuke the entire data source at this locator
                relative_dirty_locators[i] = HdDataSourceLocatorSet::universal();
                dirty_locators.insert(locator);
                continue;
            }

            // Compute relative dirty locators by intersecting and removing first element
            let mut relative_set = HdDataSourceLocatorSet::new();
            for dirty_loc in entry.dirty_locators.iter() {
                if dirty_loc.has_prefix(&locator) && dirty_loc.len() > locator.len() {
                    relative_set.insert(dirty_loc.remove_first());
                }
            }

            // Let the provider expand locators for descendants
            self.data_source_providers[i].compute_dirty_locators_for_descendants(&mut relative_set);

            if relative_set.contains(&HdDataSourceLocator::empty()) {
                // Provider expanded to universal set - nuke entire data source
                dirty_locators.insert(locator);
                relative_dirty_locators[i] = relative_set;
                continue;
            }

            // Make relative locators absolute
            for rel_loc in relative_set.iter() {
                dirty_locators.insert(locator.append_locator(rel_loc));
            }
            relative_dirty_locators[i] = relative_set;
        }

        if !dirty_locators.is_empty() {
            self.dirty_hierarchy(
                &entry.prim_path,
                &relative_dirty_locators,
                &dirty_locators,
                additional_dirty,
            );
        }

        // Check if the prim-level container itself needs invalidation
        // (container sentinel locator)
        let container_locator =
            HdDataSourceLocator::from_token(TfToken::new("__containerDataSource"));
        if entry.dirty_locators.contains(&container_locator) {
            {
                let prims = self.prims.read();
                if let Some(prim) = prims.get(&entry.prim_path) {
                    if let Some(ref ds) = prim.data_source {
                        if let Some(wrapping_ds) =
                            ds.as_any().downcast_ref::<PrimLevelWrappingDataSource>()
                        {
                            wrapping_ds.prim_container_dirtied();
                        }
                    }
                }
            }
        }
    }
}

impl HdSceneIndexBase for HdFlatteningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        // Match OpenUSD: check the primary hierarchy cache first.
        let prims = self.prims.read();
        let has_primary_entry = prims.contains_key(prim_path);
        if let Some(cached) = prims.get(prim_path) {
            if cached.data_source.is_some() {
                return cached.clone();
            }
        }
        drop(prims);

        // Then check the recent-prim cache.
        {
            let recent = self.recent_prims.read();
            if let Some(cached) = recent.get(prim_path) {
                return cached.clone();
            }
        }

        // Query input scene
        let input_prim = if let Some(input) = self.filtering_base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::empty()
        };

        // OpenUSD only wraps a missing input prim when the primary table already
        // has an entry for this exact path.
        if !input_prim.is_defined() && !has_primary_entry {
            return input_prim;
        }

        // Wrap the input data source with flattening logic.
        // Even null input data sources get wrapped to support dirtying
        // down non-contiguous hierarchy.
        let self_weak = self.self_weak.clone().unwrap_or_else(|| {
            log::warn!("[flattening] self_weak is None for prim {}", prim_path);
            Weak::new()
        });
        let wrapping_ds = PrimLevelWrappingDataSource::new(
            self_weak,
            prim_path.clone(),
            &input_prim,
            self.data_source_names.clone(),
            self.data_source_providers.clone(),
        );

        let wrapped_prim = HdSceneIndexPrim {
            prim_type: input_prim.prim_type,
            data_source: Some(wrapping_ds as HdContainerDataSourceHandle),
        };

        // Match OpenUSD: store newly wrapped prims only in the recent cache.
        let mut recent = self.recent_prims.write();
        if let Some(existing) = recent.get(prim_path) {
            return existing.clone();
        }
        recent.insert(prim_path.clone(), wrapped_prim.clone());
        wrapped_prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        // Topology is unchanged - delegate to input
        if let Some(input) = self.filtering_base.get_input_scene() {
            {
                let input_lock = input.read();
                return input_lock.get_child_prim_paths(prim_path);
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
            "HdFlatteningSceneIndex".to_string()
        } else {
            name.to_string()
        }
    }

    /// G2: SystemMessage recursion through input scenes.
    fn get_input_scenes_for_system_message(&self) -> Vec<super::base::HdSceneIndexHandle> {
        self.filtering_base
            .get_input_scene()
            .cloned()
            .into_iter()
            .collect()
    }
}

impl FilteringObserverTarget for HdFlatteningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.consolidate_recent_prims();
        let mut additional_dirty = Vec::new();

        // Dirty hierarchy for ancestors of added prims
        for entry in entries {
            self.dirty_hierarchy(
                &entry.prim_path,
                &self.relative_data_source_locators,
                &self.data_source_locator_set,
                &mut additional_dirty,
            );
        }

        // Match OpenUSD: keep the prim-table entry but drop its cached wrapper so
        // the next `get_prim()` rebuilds it from the new input prim.
        {
            let mut prims = self.prims.write();
            for entry in entries {
                if let Some(existing) = prims.get_mut(&entry.prim_path) {
                    existing.data_source = None;
                }
            }
        }

        // Forward PrimsAdded
        self.filtering_base.forward_prims_added(self, entries);

        // Send additional dirty entries for descendants
        if !additional_dirty.is_empty() {
            self.filtering_base
                .forward_prims_dirtied(self, &additional_dirty);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.consolidate_recent_prims();
        {
            let mut prims = self.prims.write();
            for entry in entries {
                if entry.prim_path.is_absolute_root_path() {
                    prims.clear();
                } else {
                    // G8: Use BTreeMap range for efficient subtree removal
                    let to_remove: Vec<SdfPath> = prims
                        .range(entry.prim_path.clone()..)
                        .take_while(|(k, _)| k.has_prefix(&entry.prim_path))
                        .map(|(k, _)| k.clone())
                        .collect();
                    for key in to_remove {
                        prims.remove(&key);
                    }
                }
            }
        }
        // G6: Clear recent cache for removed prims
        {
            let mut recent = self.recent_prims.write();
            for entry in entries {
                if entry.prim_path.is_absolute_root_path() {
                    recent.clear();
                } else {
                    recent.retain(|k, _| !k.has_prefix(&entry.prim_path));
                }
            }
        }

        self.filtering_base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.consolidate_recent_prims();
        let mut additional_dirty = Vec::new();

        for entry in entries {
            self.process_prim_dirtied(entry, &mut additional_dirty);
        }

        if flo_debug_enabled() {
            let mut input_counts: HashMap<SdfPath, usize> = HashMap::new();
            for entry in entries {
                *input_counts.entry(entry.prim_path.clone()).or_insert(0) += 1;
            }
            let mut additional_counts: HashMap<SdfPath, usize> = HashMap::new();
            for entry in &additional_dirty {
                *additional_counts
                    .entry(entry.prim_path.clone())
                    .or_insert(0) += 1;
            }
            let input_duplicates = input_counts.values().filter(|&&n| n > 1).count();
            let additional_duplicates = additional_counts.values().filter(|&&n| n > 1).count();
            let additional_duplicate_instances: usize = additional_counts
                .values()
                .map(|&n| n.saturating_sub(1))
                .sum();
            let overlap_paths: Vec<String> = additional_counts
                .keys()
                .filter(|path| input_counts.contains_key(*path))
                .map(|path| path.to_string())
                .take(8)
                .collect();
            eprintln!(
                "[flattening-debug] sender={} input_total={} input_unique={} input_dup_paths={} additional_total={} additional_unique={} additional_dup_paths={} additional_dup_instances={} overlap_with_input={} overlap_sample={:?}",
                sender.get_display_name(),
                entries.len(),
                input_counts.len(),
                input_duplicates,
                additional_dirty.len(),
                additional_counts.len(),
                additional_duplicates,
                additional_duplicate_instances,
                additional_counts
                    .keys()
                    .filter(|path| input_counts.contains_key(*path))
                    .count(),
                overlap_paths,
            );
        }

        if entries.len() >= 500 || additional_dirty.len() >= 500 {
            let first_path = entries
                .first()
                .map(|entry| entry.prim_path.to_string())
                .unwrap_or_else(|| "<none>".to_string());
            let sender_name = sender.get_display_name();
            log::info!(
                "[flattening] on_prims_dirtied in={} additional={} sender={} first={}",
                entries.len(),
                additional_dirty.len(),
                sender_name,
                first_path
            );
        }

        if additional_dirty.is_empty() {
            self.filtering_base.forward_prims_dirtied(self, entries);
        } else {
            // Combine original + additional dirty entries
            let mut combined = entries.to_vec();
            combined.extend(additional_dirty);
            self.filtering_base.forward_prims_dirtied(self, &combined);
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.consolidate_recent_prims();
        // Invalidate both old and new hierarchies
        {
            let mut prims = self.prims.write();
            for entry in entries {
                // G8: Use BTreeMap range for efficient subtree removal
                for prefix in [&entry.old_prim_path, &entry.new_prim_path] {
                    let to_remove: Vec<SdfPath> = prims
                        .range(prefix.clone()..)
                        .take_while(|(k, _)| k.has_prefix(prefix))
                        .map(|(k, _)| k.clone())
                        .collect();
                    for key in to_remove {
                        prims.remove(&key);
                    }
                }
            }
        }
        // G6: Clear recent cache
        {
            let mut recent = self.recent_prims.write();
            for entry in entries {
                recent.retain(|k, _| {
                    !k.has_prefix(&entry.old_prim_path) && !k.has_prefix(&entry.new_prim_path)
                });
            }
        }

        self.filtering_base.forward_prims_renamed(self, entries);
    }
}

/// Try to extract an HdFlattenedDataSourceProviderHandle from a data source.
///
/// In the full implementation, input_args maps names to typed sampled data
/// sources wrapping provider handles. Since we can't do full C++ RTTI in Rust,
/// we check via Any downcast.
fn extract_provider_from_ds(
    ds: &HdDataSourceBaseHandle,
) -> Option<HdFlattenedDataSourceProviderHandle> {
    // Try downcasting to our ProviderDataSource wrapper
    let any = ds.as_any();
    if let Some(pds) = any.downcast_ref::<ProviderDataSource>() {
        return Some(pds.provider.clone());
    }
    None
}

/// Data source wrapper for storing an HdFlattenedDataSourceProviderHandle.
///
/// Used in input_args to HdFlatteningSceneIndex::new.
/// Port of C++ HdMakeDataSourceContainingFlattenedDataSourceProvider.
pub struct ProviderDataSource {
    /// The wrapped provider
    pub provider: HdFlattenedDataSourceProviderHandle,
}

impl ProviderDataSource {
    /// Create a new provider data source.
    pub fn new(provider: HdFlattenedDataSourceProviderHandle) -> Arc<Self> {
        Arc::new(Self { provider })
    }
}

impl std::fmt::Debug for ProviderDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderDataSource").finish()
    }
}

impl HdDataSourceBase for ProviderDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(ProviderDataSource {
            provider: self.provider.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Convenience: create input_args for HdFlatteningSceneIndex from providers.
///
/// Returns a container data source mapping each name to its provider.
pub fn make_flattening_input_args(
    entries: &[(TfToken, HdFlattenedDataSourceProviderHandle)],
) -> HdContainerDataSourceHandle {
    use crate::data_source::HdRetainedContainerDataSource;
    let mut children = HashMap::new();
    for (name, provider) in entries {
        children.insert(
            name.clone(),
            ProviderDataSource::new(provider.clone()) as HdDataSourceBaseHandle,
        );
    }
    HdRetainedContainerDataSource::new(children)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flattening_scene_creation() {
        let scene = HdFlatteningSceneIndex::new(None, None);
        let scene_lock = scene.read();

        // Should work even without input
        let prim = scene_lock.get_prim(&SdfPath::absolute_root());
        assert!(!prim.is_defined());
    }

    #[test]
    fn test_get_flattened_names() {
        let scene = HdFlatteningSceneIndex::new(None, None);
        let scene_lock = scene.read();

        let names = scene_lock.get_flattened_data_source_names();
        assert_eq!(names.len(), 0); // No config
    }

    #[test]
    fn test_self_weak_is_set() {
        let scene = HdFlatteningSceneIndex::new(None, None);
        let scene_lock = scene.read();
        assert!(scene_lock.self_weak.is_some());
        assert!(scene_lock.self_weak.as_ref().unwrap().upgrade().is_some());
    }

    #[test]
    fn test_insert_unique_names() {
        let src = vec![TfToken::new("a"), TfToken::new("b"), TfToken::new("c")];
        let mut result = vec![TfToken::new("b"), TfToken::new("d")];
        insert_unique_names(&src, &mut result);
        assert_eq!(result.len(), 4);
        assert!(result.contains(&TfToken::new("a")));
        assert!(result.contains(&TfToken::new("c")));
    }

    #[test]
    fn test_flattening_with_providers() {
        use crate::flattened_primvars_data_source_provider::HdFlattenedPrimvarsDataSourceProvider;

        let provider: HdFlattenedDataSourceProviderHandle =
            Arc::new(HdFlattenedPrimvarsDataSourceProvider::new());

        let input_args = make_flattening_input_args(&[(TfToken::new("primvars"), provider)]);

        let scene = HdFlatteningSceneIndex::new(None, Some(input_args));
        let scene_lock = scene.read();
        let names = scene_lock.get_flattened_data_source_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].as_str(), "primvars");
    }

    #[test]
    fn test_child_prim_paths_passthrough() {
        let scene = HdFlatteningSceneIndex::new(None, None);
        let scene_lock = scene.read();

        let children = scene_lock.get_child_prim_paths(&SdfPath::absolute_root());
        assert!(children.is_empty());
    }

    #[test]
    fn test_display_name() {
        let scene = HdFlatteningSceneIndex::new(None, None);
        let scene_lock = scene.read();
        assert_eq!(scene_lock.get_display_name(), "HdFlatteningSceneIndex");
    }
}
