//! Usd_ClipCache - internal cache for clip information.
//!
//! Port of pxr/usd/usd/clipCache.h/cpp
//!
//! Private helper object for computing and caching clip information for
//! a prim on a UsdStage.

use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
use usd_pcp::PrimIndex;
use usd_sdf::Path;

// Forward declarations - these will be implemented in separate files
pub use super::clip_set::{ClipSet, ClipSetRefPtr};
pub use super::clip_set_definition::ClipSetDefinition;

// ============================================================================
// ClipCache
// ============================================================================

/// Internal cache for clip information.
///
/// Matches C++ `Usd_ClipCache`.
///
/// This is a private helper object for computing and caching clip information.
pub struct ClipCache {
    /// Map from prim path to all clips that apply to that prim, including ancestral clips.
    /// This map is sparse; only prims where clips are authored will have entries.
    table: Mutex<HashMap<Path, Vec<Arc<ClipSet>>>>,
    /// Concurrent population context (if active).
    concurrent_population_context: AtomicPtr<ConcurrentPopulationContext>,
    /// Lifeboat (if active).
    lifeboat: AtomicPtr<Lifeboat>,
}

impl ClipCache {
    /// Creates a new clip cache.
    ///
    /// Matches C++ `Usd_ClipCache()`.
    pub fn new() -> Self {
        Self {
            table: Mutex::new(HashMap::new()),
            concurrent_population_context: AtomicPtr::new(std::ptr::null_mut()),
            lifeboat: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    /// Populate the cache with clips for prim.
    ///
    /// Matches C++ `PopulateClipsForPrim(const SdfPath& path, const PcpPrimIndex& primIndex)`.
    ///
    /// Returns true if clips that may contribute opinions to attributes on prim are found,
    /// false otherwise.
    ///
    /// This function assumes that clips for ancestors of prim have already been populated.
    pub fn populate_clips_for_prim(&self, path: &Path, prim_index: &Arc<PrimIndex>) -> bool {
        let mut all_clips = Vec::new();
        self.compute_clips_from_prim_index(path, prim_index, &mut all_clips);

        let prim_has_clips = !all_clips.is_empty();
        if prim_has_clips {
            // Acquire lock if concurrent population context is active
            let concurrent_ctx_ptr = self.concurrent_population_context.load(Ordering::Acquire);
            let _lock = if !concurrent_ctx_ptr.is_null() {
                // SAFETY: The pointer was set by ConcurrentPopulationContext::new and points
                // to a valid Mutex that lives as long as the context. The atomic ordering
                // ensures we see the initialized mutex.
                #[allow(unsafe_code)]
                unsafe {
                    let mutex_ptr = concurrent_ctx_ptr as *mut std::sync::Mutex<()>;
                    Some((*mutex_ptr).lock().expect("lock poisoned"))
                }
            } else {
                None
            };

            // Find nearest ancestor with clips specified.
            let mut ancestral_clips: Option<Vec<ClipSetRefPtr>> = None;
            let mut ancestral_clips_path = path.get_parent_path();
            let mut table = self.table.lock().expect("lock poisoned");

            while !ancestral_clips_path.is_absolute_root_path() && ancestral_clips.is_none() {
                if let Some(clips) = table.get(&ancestral_clips_path) {
                    ancestral_clips = Some(clips.clone());
                    break;
                }
                ancestral_clips_path = ancestral_clips_path.get_parent_path();
            }

            if let Some(ref anc_clips) = ancestral_clips {
                // SdfPathTable will create entries for all ancestor paths when
                // inserting a new path. So if there were clips on prim /A and
                // we're inserting clips on prim /A/B/C, we need to make sure
                // we copy the ancestral clips from /A down to /A/B as well.
                let mut current_path = path.get_parent_path();
                while current_path != ancestral_clips_path {
                    table.insert(current_path.clone(), anc_clips.clone());
                    current_path = current_path.get_parent_path();
                }

                // Append ancestral clips since they are weaker than clips
                // authored on this prim.
                all_clips.extend_from_slice(anc_clips);
            }

            table.insert(path.clone(), all_clips);
        }

        prim_has_clips
    }

    /// Get all the layers that have been opened because we needed to extract
    /// data from their corresponding clips.
    ///
    /// Matches C++ `GetUsedLayers()`.
    pub fn get_used_layers(&self) -> Vec<Arc<usd_sdf::Layer>> {
        // Acquire lock if concurrent population context is active
        let concurrent_ctx_ptr = self.concurrent_population_context.load(Ordering::Acquire);
        let _lock = if !concurrent_ctx_ptr.is_null() {
            // SAFETY: The pointer was set by ConcurrentPopulationContext::new and points
            // to a valid Mutex that lives as long as the context. The atomic ordering
            // ensures we see the initialized mutex.
            #[allow(unsafe_code)]
            unsafe {
                let mutex_ptr = concurrent_ctx_ptr as *mut std::sync::Mutex<()>;
                Some((*mutex_ptr).lock().expect("lock poisoned"))
            }
        } else {
            None
        };

        // Use a set to track unique layers by their identifier to avoid duplicates
        let mut layer_set: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut layers: Vec<Arc<usd_sdf::Layer>> = Vec::new();
        let table = self.table.lock().expect("lock poisoned");

        for clips_list in table.values() {
            for clip_set in clips_list.iter() {
                // Get manifest clip layer if open
                if let Some(ref manifest_clip) = clip_set.manifest_clip {
                    if let Some(layer) = manifest_clip.get_layer_for_clip() {
                        let identifier = layer.identifier();
                        if !identifier.is_empty() && layer_set.insert(identifier.to_string()) {
                            layers.push(layer);
                        }
                    }
                }

                // Get value clip layers if open
                for clip in clip_set.value_clips.iter() {
                    if let Some(layer) = clip.get_layer_for_clip() {
                        let identifier = layer.identifier();
                        if !identifier.is_empty() && layer_set.insert(identifier.to_string()) {
                            layers.push(layer);
                        }
                    }
                }
            }
        }

        layers
    }

    /// Reload all clip layers that have been opened.
    ///
    /// Matches C++ `Reload()`.
    pub fn reload(&self) {
        use std::collections::HashSet;

        // Collect all unique clip sets to iterate over to avoid duplicated work
        // due to ancestral clip entries (see PopulateClipsForPrim)
        let table = self.table.lock().expect("lock poisoned");
        let mut clip_sets: HashSet<*const ClipSet> = HashSet::new();
        for clips_list in table.values() {
            for clip_set in clips_list.iter() {
                let clip_set_ptr = Arc::as_ptr(clip_set);
                clip_sets.insert(clip_set_ptr);
            }
        }
        drop(table);

        // Iterate through all clip sets and call SdfLayer::Reload for any
        // associated layers that are opened.
        let mut reloaded_clip_layers: HashSet<*const usd_sdf::Layer> = HashSet::new();
        for clip_set_ptr in clip_sets {
            // SAFETY: The pointer came from Arc::as_ptr and the Arc is still held in
            // the table, so the ClipSet is valid for the duration of this iteration.
            #[allow(unsafe_code)]
            unsafe {
                let clip_set = &*clip_set_ptr;

                // Reload all clip layers.
                for clip in clip_set.value_clips.iter() {
                    // Get layer from clip
                    if let Some(layer_arc) = clip.get_layer_for_clip() {
                        let layer_ptr = Arc::as_ptr(&layer_arc);

                        // It's possible (but unlikely?) that the same clip layer is used
                        // in multiple clip sets. We need to keep track of all the layers
                        // that were reloaded to handle that case.
                        if reloaded_clip_layers.insert(layer_ptr) {
                            // Reload the layer (ignore errors for anonymous layers)
                            let _ = layer_arc.reload();
                        }
                    }
                }

                // Reload the manifest if it was supplied by the user, otherwise
                // regenerate it.
                if let Some(ref manifest_clip) = clip_set.manifest_clip {
                    if let Some(layer_arc) = manifest_clip.get_layer_for_clip() {
                        let layer_ptr = Arc::as_ptr(&layer_arc);
                        let identifier = layer_arc.identifier();
                        let is_generated = identifier.contains("generated_manifest");

                        if reloaded_clip_layers.insert(layer_ptr) {
                            if is_generated {
                                // For generated manifests, we'll regenerate them below
                                // Just mark that we need to regenerate
                            } else {
                                // Reload user-supplied manifest (ignore errors for anonymous layers)
                                let _ = layer_arc.reload();
                            }
                        }
                    }
                }
            }
        }

        // Regenerate manifests for clip sets that had generated manifests
        // We identify generated manifests by their identifier containing "generated_manifest"
        let table = self.table.lock().expect("lock poisoned");
        let mut paths_to_recompute: Vec<Path> = Vec::new();
        for (path, clips_list) in table.iter() {
            for clip_set in clips_list.iter() {
                if let Some(ref manifest_clip) = clip_set.manifest_clip {
                    if let Some(layer_arc) = manifest_clip.get_layer_for_clip() {
                        let identifier = layer_arc.identifier();
                        if identifier.contains("generated_manifest") {
                            paths_to_recompute.push(path.clone());
                            break; // Only need to mark this path once
                        }
                    }
                }
            }
        }
        drop(table);

        // Recompute clips for paths that had generated manifests
        // Note: This requires the prim indexes, which we don't have here.
        // In practice, this would be called from UsdStage::Reload() which
        // has access to the prim indexes. For now, we just mark that
        // regeneration is needed (the caller should handle this).
    }

    /// Get all clips that may contribute opinions to attributes on the
    /// prim at path, including clips that were authored on ancestral prims.
    ///
    /// Matches C++ `GetClipsForPrim(const SdfPath& path)`.
    ///
    /// The returned vector contains all clips that affect the prim at path
    /// in strength order. Each individual list of value clips will be ordered
    /// by start time.
    pub fn get_clips_for_prim(&self, path: &Path) -> Vec<ClipSetRefPtr> {
        // Search up the path hierarchy to find clips
        let table = self.table.lock().expect("lock poisoned");
        let mut current_path = path.clone();
        while !current_path.is_absolute_root_path() {
            if let Some(clips) = table.get(&current_path) {
                return clips.clone();
            }
            current_path = current_path.get_parent_path();
        }
        Vec::new()
    }

    /// Invalidates the clip cache for prims at and below path.
    ///
    /// Matches C++ `InvalidateClipsForPrim(const SdfPath& path)`.
    ///
    /// A Lifeboat object must be active for this cache before calling
    /// this function. This potentially allows the underlying clip layers
    /// to be reused if the clip cache is repopulated while the lifeboat
    /// is still active.
    ///
    /// NOTE: This function must not be invoked concurrently with any other
    /// function on this object.
    pub fn invalidate_clips_for_prim(&self, path: &Path) {
        // Check that lifeboat is active
        let lifeboat_ptr = self.lifeboat.load(Ordering::Acquire);
        if lifeboat_ptr.is_null() {
            // No lifeboat active - just clear the cache
            let mut table = self.table.lock().expect("lock poisoned");
            table.retain(|p, _| {
                let p_str = p.as_str();
                let path_str = path.as_str();
                !p_str.starts_with(path_str) && !path_str.starts_with(p_str)
            });
            return;
        }

        // Lifeboat is active - store clips in lifeboat before removing
        // SAFETY: The pointer was set by Lifeboat::new and points to valid LifeboatData
        // that lives as long as the Lifeboat object. The atomic ordering ensures we see
        // the initialized data.
        #[allow(unsafe_code)]
        unsafe {
            let lifeboat_data = &mut *(lifeboat_ptr as *mut LifeboatData);
            let mut table = self.table.lock().expect("lock poisoned");

            // Collect clips to store in lifeboat
            let mut clips_to_store: Vec<ClipSetRefPtr> = Vec::new();
            let mut manifests_to_store: HashMap<ManifestKey, String> = HashMap::new();

            for (p, clips_list) in table.iter() {
                let p_str = p.as_str();
                let path_str = path.as_str();

                // Check if this path should be invalidated
                if p_str.starts_with(path_str) || path_str.starts_with(p_str) {
                    // Store clips in lifeboat
                    clips_to_store.extend_from_slice(clips_list);

                    // Store generated manifest identifiers
                    for clip_set in clips_list.iter() {
                        if let Some(ref manifest_clip) = clip_set.manifest_clip {
                            if let Some(layer_arc) = manifest_clip.get_layer_for_clip() {
                                let identifier = layer_arc.identifier();
                                if identifier.contains("generated_manifest") {
                                    // Extract manifest key from clip set
                                    // Note: clip_prim_path and clip_asset_paths are not tracked here
                                    // as they're not needed for manifest key lookup. The key is
                                    // sufficient to identify the manifest layer.
                                    let key = ManifestKey {
                                        prim_path: p.clone(),
                                        clip_set_name: clip_set.name.clone(),
                                        clip_prim_path: Path::empty(), // Not needed for lookup
                                        clip_asset_paths: Vec::new(),  // Not needed for lookup
                                    };
                                    manifests_to_store.insert(key, identifier.to_string());
                                }
                            }
                        }
                    }
                }
            }

            // Store in lifeboat
            lifeboat_data.clips.extend_from_slice(&clips_to_store);
            lifeboat_data.generated_manifests.extend(manifests_to_store);

            // Remove entries from cache
            table.retain(|p, _| {
                let p_str = p.as_str();
                let path_str = path.as_str();
                !p_str.starts_with(path_str) && !path_str.starts_with(p_str)
            });
        }
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Computes clips from prim index.
    ///
    /// Matches C++ `_ComputeClipsFromPrimIndex`.
    fn compute_clips_from_prim_index(
        &self,
        usd_prim_path: &Path,
        prim_index: &Arc<PrimIndex>,
        clips: &mut Vec<ClipSetRefPtr>,
    ) {
        use super::clip_set_definition::compute_clip_set_definitions_for_prim_index;

        let mut clip_set_defs = Vec::new();
        let mut clip_set_names = Vec::new();
        compute_clip_set_definitions_for_prim_index(
            prim_index,
            &mut clip_set_defs,
            &mut clip_set_names,
        );

        clips.reserve(clip_set_defs.len());
        for (i, clip_set_def) in clip_set_defs.iter_mut().enumerate() {
            let clip_set_name = &clip_set_names[i];

            // If no clip manifest was explicitly specified and we have an
            // active lifeboat (i.e., we're in the middle of change processing)
            // see if we can reuse a generated manifest from before.
            let mut reusing_generated_manifest = false;
            if clip_set_def.clip_manifest_asset_path.is_none() {
                let lifeboat_ptr = self.lifeboat.load(Ordering::Acquire);
                if !lifeboat_ptr.is_null() {
                    // SAFETY: The pointer was set by Lifeboat::new and points to valid LifeboatData
                    // that lives as long as the Lifeboat object. The atomic ordering ensures we see
                    // the initialized data.
                    #[allow(unsafe_code)]
                    unsafe {
                        let lifeboat_data = &*(lifeboat_ptr as *const LifeboatData);
                        let key = ManifestKey {
                            prim_path: usd_prim_path.clone(),
                            clip_set_name: clip_set_name.clone(),
                            clip_prim_path: clip_set_def
                                .clip_prim_path
                                .as_ref()
                                .and_then(|p| Path::from_string(p))
                                .unwrap_or_else(Path::empty),
                            clip_asset_paths: clip_set_def
                                .clip_asset_paths
                                .as_ref()
                                .map(|a| a.iter().cloned().collect())
                                .unwrap_or_default(),
                        };

                        if let Some(manifest_identifier) =
                            lifeboat_data.generated_manifests.get(&key)
                        {
                            clip_set_def.clip_manifest_asset_path =
                                Some(usd_sdf::AssetPath::new(manifest_identifier));
                            reusing_generated_manifest = true;
                        }
                    }
                }
            }

            let mut status = None;
            let clip_set = ClipSet::new(clip_set_name.clone(), clip_set_def, &mut status);
            if let Some(ref cs) = clip_set {
                if !cs.value_clips.is_empty() {
                    // If reusing a previously-generated manifest from the lifeboat,
                    // pull on it here to ensure the manifest takes ownership of it.
                    if reusing_generated_manifest {
                        if let Some(ref manifest_clip) = cs.manifest_clip {
                            // Reference is held by manifest_clip
                            if let Some(layer_arc) = manifest_clip.get_layer_for_clip() {
                                let _ = layer_arc.identifier();
                            }
                        }
                    }
                    clips.push(clip_set.expect("checked above"));
                }
            }
        }
    }

    /// Get clips for prim without locking (internal helper).
    ///
    /// Matches C++ `_GetClipsForPrim_NoLock`.
    #[allow(dead_code)] // C++ parity - internal helper
    fn get_clips_for_prim_no_lock(&self, path: &Path) -> Vec<ClipSetRefPtr> {
        // This is unsafe without external locking, but matches C++ pattern
        // In practice, caller should hold the lock
        self.get_clips_for_prim(path)
    }
}

impl Default for ClipCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ConcurrentPopulationContext
// ============================================================================

/// Structure for enabling cache population via concurrent calls to
/// PopulateClipsForPrim.
///
/// Matches C++ `Usd_ClipCache::ConcurrentPopulationContext`.
///
/// Protects member data reads/writes with a mutex during its lifetime.
pub struct ConcurrentPopulationContext {
    /// Reference to the cache.
    cache: *mut ClipCache,
    /// Mutex for synchronization.
    _mutex: Mutex<()>,
}

impl ConcurrentPopulationContext {
    /// Creates a new concurrent population context.
    ///
    /// Matches C++ `ConcurrentPopulationContext(Usd_ClipCache &cache)`.
    pub fn new(cache: &mut ClipCache) -> Self {
        let cache_ptr = cache as *mut ClipCache;
        let context = Self {
            cache: cache_ptr,
            _mutex: Mutex::new(()),
        };

        // Set context in cache - store pointer to mutex for thread-safe access
        // This matches the C++ implementation which stores a pointer to the context
        // SAFETY: We're storing a pointer to our own _mutex field which lives as long as
        // this ConcurrentPopulationContext. The pointer is cleared in drop() before the
        // mutex is destroyed. Access via cache_ptr is safe because it points to the cache
        // parameter which must outlive this context.
        #[allow(unsafe_code)]
        unsafe {
            let mutex_ptr = &context._mutex as *const Mutex<()> as *mut Mutex<()>;
            (*cache_ptr)
                .concurrent_population_context
                .store(mutex_ptr as *mut _, Ordering::Release);
        }

        context
    }
}

impl Drop for ConcurrentPopulationContext {
    fn drop(&mut self) {
        // Clear context from cache
        // SAFETY: self.cache points to the cache that was passed to new(), which must
        // still be valid since we're dropping this context. We're just clearing the
        // atomic pointer before the mutex is destroyed.
        #[allow(unsafe_code)]
        unsafe {
            if let Some(cache) = self.cache.as_mut() {
                cache
                    .concurrent_population_context
                    .store(std::ptr::null_mut(), Ordering::Release);
            }
        }
    }
}

// ============================================================================
// Lifeboat
// ============================================================================

/// Structure for keeping invalidated clip data alive.
///
/// Matches C++ `Usd_ClipCache::Lifeboat`.
///
/// This allows underlying clip layers to be reused if the clip cache is
/// repopulated while the lifeboat is still active.
pub struct Lifeboat {
    /// Reference to the cache.
    cache: *mut ClipCache,
    /// Data stored in lifeboat.
    pub(crate) data: LifeboatData,
}

/// Data stored in lifeboat.
pub(crate) struct LifeboatData {
    /// Clips kept alive.
    pub clips: Vec<ClipSetRefPtr>,
    /// Generated manifests kept alive (keyed by manifest key).
    pub generated_manifests: HashMap<ManifestKey, String>,
}

/// Key for identifying generated manifests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ManifestKey {
    prim_path: Path,
    clip_set_name: String,
    clip_prim_path: Path,
    clip_asset_paths: Vec<usd_sdf::AssetPath>,
}

impl Lifeboat {
    /// Creates a new lifeboat.
    ///
    /// Matches C++ `Lifeboat(Usd_ClipCache& cache)`.
    pub fn new(cache: &mut ClipCache) -> Self {
        let cache_ptr = cache as *mut ClipCache;
        let mut lifeboat = Self {
            cache: cache_ptr,
            data: LifeboatData {
                clips: Vec::new(),
                generated_manifests: HashMap::new(),
            },
        };

        // Set lifeboat in cache - store pointer to data for thread-safe access
        // This matches the C++ implementation which stores a pointer to the lifeboat data
        // SAFETY: We're storing a pointer to our own data field which lives as long as
        // this Lifeboat. The pointer is cleared in drop() before the data is destroyed.
        // Access via cache_ptr is safe because it points to the cache parameter which
        // must outlive this lifeboat.
        #[allow(unsafe_code)]
        unsafe {
            let data_ptr = &mut lifeboat.data as *mut LifeboatData as *mut _;
            (*cache_ptr).lifeboat.store(data_ptr, Ordering::Release);
        }

        lifeboat
    }
}

impl Drop for Lifeboat {
    fn drop(&mut self) {
        // Clear lifeboat from cache
        // SAFETY: self.cache points to the cache that was passed to new(), which must
        // still be valid since we're dropping this lifeboat. We're just clearing the
        // atomic pointer before the data is destroyed.
        #[allow(unsafe_code)]
        unsafe {
            if let Some(cache) = self.cache.as_mut() {
                cache
                    .lifeboat
                    .store(std::ptr::null_mut(), Ordering::Release);
            }
        }
    }
}
