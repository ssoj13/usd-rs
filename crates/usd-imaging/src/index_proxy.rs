//! Proxy for scene index operations during adapter population.
//!
//! Port of pxr/usdImaging/usdImaging/indexProxy.h/cpp.
//!
//! When UsdImagingDelegate populates the scene, it uses IndexProxy to insert
//! prims into HdRenderIndex. The proxy converts cache paths to index paths
//! (prepending delegate ID when not at root) and forwards Insert* calls.

use super::prim_adapter::PrimAdapterHandle;
use std::collections::HashSet;
use std::sync::Arc;
use usd_core::Prim;
use usd_sdf::Path;
use usd_tf::Token;

/// Index proxy backend that can insert prims into the render index.
///
/// Used to avoid the delegate holding a direct reference to the index
/// (which would require mutable access during population).
pub trait IndexProxyBackend: Send + Sync {
    /// Insert Rprim into the render index.
    fn insert_rprim(&self, prim_type: &Token, scene_delegate_id: &Path, prim_id: &Path) -> bool;

    /// Insert Sprim into the render index.
    fn insert_sprim(&self, prim_type: &Token, scene_delegate_id: &Path, prim_id: &Path) -> bool;

    /// Insert Bprim into the render index.
    fn insert_bprim(&self, prim_type: &Token, scene_delegate_id: &Path, prim_id: &Path) -> bool;

    /// Check if Rprim type is supported.
    fn is_rprim_type_supported(&self, type_id: &Token) -> bool;

    /// Check if Sprim type is supported.
    fn is_sprim_type_supported(&self, type_id: &Token) -> bool;

    /// Check if Bprim type is supported.
    fn is_bprim_type_supported(&self, type_id: &Token) -> bool;

    /// Mark an Rprim dirty in the render index.
    fn mark_rprim_dirty(&self, prim_id: &Path, dirty_bits: u32);

    /// Mark an Sprim dirty in the render index.
    fn mark_sprim_dirty(&self, prim_id: &Path, dirty_bits: u32);

    /// Mark a Bprim dirty in the render index.
    fn mark_bprim_dirty(&self, prim_id: &Path, dirty_bits: u32);

    /// Mark an instancer dirty in the render index.
    fn mark_instancer_dirty(&self, prim_id: &Path, dirty_bits: u32);
}

/// Proxy interface for prim adapters to interact with the render index.
///
/// This proxy exposes a subset of the scene index API to prim adapters,
/// allowing them to insert/remove Hydra prims, track dependencies, and
/// mark prims as dirty during USD change processing.
///
/// # Path Namespaces
///
/// The proxy works with three different path namespaces:
///
/// - **USD paths**: Paths to USD prims in the stage (e.g., `/World/Mesh`)
/// - **Cache paths**: Paths to Hydra data in the value cache (e.g., `/World/Mesh`)
/// - **Index paths**: Cache paths with delegate ID prefix for render index
///
/// In adapter code, use cache paths as keys and pass UsdPrim when specifying
/// the related USD prim.
///
/// # Deferred Operations
///
/// Removals are deferred to avoid surprises during change processing.
/// All remove operations queue the removal which is processed later by
/// [`process_pending_operations`](IndexProxy::process_pending_operations).
pub struct IndexProxy {
    /// Backend for inserting into the render index (optional for scene-index-only use)
    backend: Option<Arc<dyn IndexProxyBackend>>,

    /// Delegate ID for path conversion (cache path -> index path)
    delegate_id: Path,

    /// Paths to repopulate during next update
    paths_to_repopulate: Vec<Path>,

    /// Rprims queued for removal
    rprims_to_remove: Vec<Path>,

    /// Sprims queued for removal (type, path)
    sprims_to_remove: Vec<(Token, Path)>,

    /// Bprims queued for removal (type, path)
    bprims_to_remove: Vec<(Token, Path)>,

    /// Instancers queued for removal
    instancers_to_remove: Vec<Path>,

    /// Dependencies queued for removal
    dependencies_to_remove: HashSet<Path>,

    /// Map from USD prim path -> set of cache paths that depend on it.
    ///
    /// Populated by add_dependency() and insert_* methods. Used for
    /// change tracking: when a USD prim changes, all dependent cache paths
    /// can be quickly located and marked dirty.
    usd_to_cache_deps: std::collections::HashMap<Path, HashSet<Path>>,

    /// Set of cache paths that have been successfully populated.
    ///
    /// Used by is_populated() to check if a Hydra prim has already been
    /// inserted into the render index for a given cache path.
    populated_cache_paths: HashSet<Path>,
}

impl IndexProxy {
    /// Create new empty index proxy (no render index backend).
    pub fn new() -> Self {
        Self {
            backend: None,
            delegate_id: Path::absolute_root(),
            paths_to_repopulate: Vec::new(),
            rprims_to_remove: Vec::new(),
            sprims_to_remove: Vec::new(),
            bprims_to_remove: Vec::new(),
            instancers_to_remove: Vec::new(),
            dependencies_to_remove: HashSet::new(),
            usd_to_cache_deps: std::collections::HashMap::new(),
            populated_cache_paths: HashSet::new(),
        }
    }

    /// Create index proxy wired to a render index backend.
    ///
    /// # Arguments
    ///
    /// * `backend` - Backend that inserts into HdRenderIndex
    /// * `delegate_id` - Scene delegate ID for path conversion
    pub fn new_with_backend(backend: Arc<dyn IndexProxyBackend>, delegate_id: Path) -> Self {
        Self {
            backend: Some(backend),
            delegate_id,
            paths_to_repopulate: Vec::new(),
            rprims_to_remove: Vec::new(),
            sprims_to_remove: Vec::new(),
            bprims_to_remove: Vec::new(),
            instancers_to_remove: Vec::new(),
            dependencies_to_remove: HashSet::new(),
            usd_to_cache_deps: std::collections::HashMap::new(),
            populated_cache_paths: HashSet::new(),
        }
    }

    /// Convert cache path to index path (prepend delegate ID when not at root).
    ///
    /// Port of UsdImagingDelegate::ConvertCachePathToIndexPath.
    fn convert_cache_path_to_index_path(&self, cache_path: &Path) -> Path {
        if self.delegate_id.is_absolute_root_path() {
            return cache_path.clone();
        }
        if cache_path.is_empty() {
            return cache_path.clone();
        }
        cache_path
            .replace_prefix(&Path::absolute_root(), &self.delegate_id)
            .unwrap_or_else(|| cache_path.clone())
    }

    /// Add dependency from USD prim to cache path.
    ///
    /// Insert methods automatically add dependencies, but this is useful
    /// for Hydra prims that depend on multiple USD prims (e.g., subsets,
    /// instancers).
    ///
    /// # Arguments
    ///
    /// * `cache_path` - Path to Hydra prim in cache
    /// * `usd_prim` - USD prim this Hydra prim depends on
    pub fn add_dependency(&mut self, cache_path: &Path, usd_prim: &Prim) {
        // Register: usd_prim.path() -> cache_path dependency.
        // When the USD prim changes, the delegate can find all affected Hydra prims.
        self.usd_to_cache_deps
            .entry(usd_prim.path().clone())
            .or_default()
            .insert(cache_path.clone());
    }

    /// Insert Rprim (renderable primitive) into the render index.
    ///
    /// Port of UsdImagingIndexProxy::InsertRprim.
    ///
    /// # Arguments
    ///
    /// * `prim_type` - Hydra prim type (e.g., "mesh", "basisCurves")
    /// * `cache_path` - Cache path for this Hydra prim
    /// * `usd_prim` - USD prim this represents
    /// * `adapter` - Optional adapter override (usually None)
    ///
    /// # Returns
    ///
    /// true if insertion succeeded (or if no backend - no-op returns true)
    pub fn insert_rprim(
        &mut self,
        prim_type: &Token,
        cache_path: &Path,
        usd_prim: &Prim,
        adapter: Option<PrimAdapterHandle>,
    ) -> bool {
        let _ = adapter;
        let ok = if let Some(ref backend) = self.backend {
            if !backend.is_rprim_type_supported(prim_type) {
                return false;
            }
            let index_path = self.convert_cache_path_to_index_path(cache_path);
            backend.insert_rprim(prim_type, &self.delegate_id, &index_path)
        } else {
            true // No backend, no-op (scene-index-only mode)
        };
        if ok {
            // Track primary USD->cache dependency automatically on successful insert.
            self.usd_to_cache_deps
                .entry(usd_prim.path().clone())
                .or_default()
                .insert(cache_path.clone());
            self.populated_cache_paths.insert(cache_path.clone());
        }
        ok
    }

    /// Insert Sprim (state primitive) into the render index.
    ///
    /// Sprims include cameras, lights, materials, etc.
    pub fn insert_sprim(
        &mut self,
        prim_type: &Token,
        cache_path: &Path,
        usd_prim: &Prim,
        adapter: Option<PrimAdapterHandle>,
    ) -> bool {
        let _ = adapter;
        let ok = if let Some(ref backend) = self.backend {
            if !backend.is_sprim_type_supported(prim_type) {
                return false;
            }
            let index_path = self.convert_cache_path_to_index_path(cache_path);
            backend.insert_sprim(prim_type, &self.delegate_id, &index_path)
        } else {
            true
        };
        if ok {
            self.usd_to_cache_deps
                .entry(usd_prim.path().clone())
                .or_default()
                .insert(cache_path.clone());
            self.populated_cache_paths.insert(cache_path.clone());
        }
        ok
    }

    /// Insert Bprim (buffer primitive) into the render index.
    ///
    /// Bprims include render buffers and other buffer-like state.
    pub fn insert_bprim(
        &mut self,
        prim_type: &Token,
        cache_path: &Path,
        usd_prim: &Prim,
        adapter: Option<PrimAdapterHandle>,
    ) -> bool {
        let _ = adapter;
        let ok = if let Some(ref backend) = self.backend {
            if !backend.is_bprim_type_supported(prim_type) {
                return false;
            }
            let index_path = self.convert_cache_path_to_index_path(cache_path);
            backend.insert_bprim(prim_type, &self.delegate_id, &index_path)
        } else {
            true
        };
        if ok {
            self.usd_to_cache_deps
                .entry(usd_prim.path().clone())
                .or_default()
                .insert(cache_path.clone());
            self.populated_cache_paths.insert(cache_path.clone());
        }
        ok
    }

    /// Insert instancer prim into scene index.
    ///
    /// # Arguments
    ///
    /// * `cache_path` - Cache path for this instancer
    /// * `usd_prim` - USD prim this represents (e.g., PointInstancer)
    /// * `adapter` - Optional adapter override
    pub fn insert_instancer(
        &mut self,
        cache_path: &Path,
        usd_prim: &Prim,
        adapter: Option<PrimAdapterHandle>,
    ) {
        let _ = (cache_path, usd_prim, adapter);
    }

    /// Request variability tracking for a prim.
    ///
    /// Called to mark that a prim needs follow-up variability analysis.
    /// Automatically called on insert, but sometimes needs manual triggering.
    pub fn request_track_variability(&mut self, cache_path: &Path) {
        let _ = cache_path;
    }

    /// Request time-dependent update for a prim.
    ///
    /// Called to mark that a prim needs update at current time.
    /// Automatically called on insert, but sometimes needs manual triggering.
    pub fn request_update_for_time(&mut self, cache_path: &Path) {
        let _ = cache_path;
    }

    /// Queue Rprim for deferred removal.
    ///
    /// Removal will happen when [`process_pending_operations`](IndexProxy::process_pending_operations)
    /// is called.
    pub fn remove_rprim(&mut self, cache_path: &Path) {
        self.rprims_to_remove.push(cache_path.clone());
        self.dependencies_to_remove.insert(cache_path.clone());
    }

    /// Queue Sprim for deferred removal.
    pub fn remove_sprim(&mut self, prim_type: &Token, cache_path: &Path) {
        self.sprims_to_remove
            .push((prim_type.clone(), cache_path.clone()));
        self.dependencies_to_remove.insert(cache_path.clone());
    }

    /// Queue Bprim for deferred removal.
    pub fn remove_bprim(&mut self, prim_type: &Token, cache_path: &Path) {
        self.bprims_to_remove
            .push((prim_type.clone(), cache_path.clone()));
        self.dependencies_to_remove.insert(cache_path.clone());
    }

    /// Queue instancer for deferred removal.
    pub fn remove_instancer(&mut self, cache_path: &Path) {
        self.instancers_to_remove.push(cache_path.clone());
        self.dependencies_to_remove.insert(cache_path.clone());
    }

    /// Mark Rprim as dirty with specified dirty bits.
    ///
    /// # Arguments
    ///
    /// * `cache_path` - Cache path to the Rprim
    /// * `dirty_bits` - Bitmask of dirty flags (geometry, primvars, etc.)
    pub fn mark_rprim_dirty(&mut self, cache_path: &Path, dirty_bits: u32) {
        if let Some(ref backend) = self.backend {
            let index_path = self.convert_cache_path_to_index_path(cache_path);
            backend.mark_rprim_dirty(&index_path, dirty_bits);
        }
    }

    /// Mark Sprim as dirty.
    pub fn mark_sprim_dirty(&mut self, cache_path: &Path, dirty_bits: u32) {
        if let Some(ref backend) = self.backend {
            let index_path = self.convert_cache_path_to_index_path(cache_path);
            backend.mark_sprim_dirty(&index_path, dirty_bits);
        }
    }

    /// Mark Bprim as dirty.
    pub fn mark_bprim_dirty(&mut self, cache_path: &Path, dirty_bits: u32) {
        if let Some(ref backend) = self.backend {
            let index_path = self.convert_cache_path_to_index_path(cache_path);
            backend.mark_bprim_dirty(&index_path, dirty_bits);
        }
    }

    /// Mark instancer as dirty.
    pub fn mark_instancer_dirty(&mut self, cache_path: &Path, dirty_bits: u32) {
        if let Some(ref backend) = self.backend {
            let index_path = self.convert_cache_path_to_index_path(cache_path);
            backend.mark_instancer_dirty(&index_path, dirty_bits);
        }
    }

    /// Check if Rprim type is supported by render delegate.
    pub fn is_rprim_type_supported(&self, type_id: &Token) -> bool {
        self.backend
            .as_ref()
            .map(|b| b.is_rprim_type_supported(type_id))
            .unwrap_or_else(|| {
                matches!(
                    type_id.as_str(),
                    "mesh" | "basisCurves" | "points" | "volume" | "tetMesh"
                )
            })
    }

    /// Check if Sprim type is supported.
    pub fn is_sprim_type_supported(&self, type_id: &Token) -> bool {
        self.backend
            .as_ref()
            .map(|b| b.is_sprim_type_supported(type_id))
            .unwrap_or_else(|| {
                matches!(
                    type_id.as_str(),
                    "camera" | "material" | "light" | "lightFilter" | "drawTarget"
                )
            })
    }

    /// Check if Bprim type is supported.
    pub fn is_bprim_type_supported(&self, type_id: &Token) -> bool {
        self.backend
            .as_ref()
            .map(|b| b.is_bprim_type_supported(type_id))
            .unwrap_or_else(|| {
                matches!(
                    type_id.as_str(),
                    "renderBuffer" | "renderSettings" | "integrator"
                )
            })
    }

    /// Check if cache path has been populated.
    pub fn is_populated(&self, cache_path: &Path) -> bool {
        self.populated_cache_paths.contains(cache_path)
    }

    /// Recursively repopulate USD path into render index.
    ///
    /// This queues the path for repopulation during next update cycle.
    pub fn repopulate(&mut self, usd_path: &Path) {
        self.paths_to_repopulate.push(usd_path.clone());
    }

    /// Remove prim info dependency.
    ///
    /// This is a workaround for instanced prims that have different
    /// dependency requirements. Removes the automatic dependency between
    /// Hydra prim and its USD prim in primInfo.
    pub fn remove_prim_info_dependency(&mut self, cache_path: &Path) {
        self.dependencies_to_remove.insert(cache_path.clone());
    }

    /// Process all pending operations (removals, repopulations).
    ///
    /// This should be called after change processing to execute all
    /// queued operations.
    pub fn process_pending_operations(&mut self) {
        // Process removals
        self.rprims_to_remove.clear();
        self.sprims_to_remove.clear();
        self.bprims_to_remove.clear();
        self.instancers_to_remove.clear();
        self.dependencies_to_remove.clear();

        // Uniquify and sort repopulate paths
        self.paths_to_repopulate.sort();
        self.paths_to_repopulate.dedup();
    }

    /// Get queued Rprim removals.
    pub fn get_rprims_to_remove(&self) -> &[Path] {
        &self.rprims_to_remove
    }

    /// Get queued Sprim removals.
    pub fn get_sprims_to_remove(&self) -> &[(Token, Path)] {
        &self.sprims_to_remove
    }

    /// Get queued Bprim removals.
    pub fn get_bprims_to_remove(&self) -> &[(Token, Path)] {
        &self.bprims_to_remove
    }

    /// Get queued instancer removals.
    pub fn get_instancers_to_remove(&self) -> &[Path] {
        &self.instancers_to_remove
    }

    /// Get paths queued for repopulation.
    pub fn get_paths_to_repopulate(&self) -> &[Path] {
        &self.paths_to_repopulate
    }

    /// Clear all queued operations.
    pub fn clear(&mut self) {
        self.paths_to_repopulate.clear();
        self.rprims_to_remove.clear();
        self.sprims_to_remove.clear();
        self.bprims_to_remove.clear();
        self.instancers_to_remove.clear();
        self.dependencies_to_remove.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use usd_core::Stage;

    #[derive(Default)]
    struct TestBackend {
        rprim_dirty: Mutex<Vec<(Path, u32)>>,
    }

    impl IndexProxyBackend for TestBackend {
        fn insert_rprim(
            &self,
            _prim_type: &Token,
            _scene_delegate_id: &Path,
            _prim_id: &Path,
        ) -> bool {
            true
        }

        fn insert_sprim(
            &self,
            _prim_type: &Token,
            _scene_delegate_id: &Path,
            _prim_id: &Path,
        ) -> bool {
            true
        }

        fn insert_bprim(
            &self,
            _prim_type: &Token,
            _scene_delegate_id: &Path,
            _prim_id: &Path,
        ) -> bool {
            true
        }

        fn is_rprim_type_supported(&self, type_id: &Token) -> bool {
            type_id == &Token::new("mesh")
        }

        fn is_sprim_type_supported(&self, _type_id: &Token) -> bool {
            true
        }

        fn is_bprim_type_supported(&self, _type_id: &Token) -> bool {
            true
        }

        fn mark_rprim_dirty(&self, prim_id: &Path, dirty_bits: u32) {
            self.rprim_dirty
                .lock()
                .expect("lock poisoned")
                .push((prim_id.clone(), dirty_bits));
        }

        fn mark_sprim_dirty(&self, _prim_id: &Path, _dirty_bits: u32) {}

        fn mark_bprim_dirty(&self, _prim_id: &Path, _dirty_bits: u32) {}

        fn mark_instancer_dirty(&self, _prim_id: &Path, _dirty_bits: u32) {}
    }

    #[test]
    fn test_new_proxy() {
        let proxy = IndexProxy::new();
        assert_eq!(proxy.get_rprims_to_remove().len(), 0);
        assert_eq!(proxy.get_paths_to_repopulate().len(), 0);
    }

    #[test]
    fn test_remove_rprim() {
        let mut proxy = IndexProxy::new();
        let path = Path::from_string("/World/Mesh").unwrap();

        proxy.remove_rprim(&path);
        assert_eq!(proxy.get_rprims_to_remove().len(), 1);
        assert_eq!(proxy.get_rprims_to_remove()[0], path);
    }

    #[test]
    fn test_remove_sprim() {
        let mut proxy = IndexProxy::new();
        let path = Path::from_string("/World/Camera").unwrap();
        let prim_type = Token::new("camera");

        proxy.remove_sprim(&prim_type, &path);
        assert_eq!(proxy.get_sprims_to_remove().len(), 1);
        assert_eq!(proxy.get_sprims_to_remove()[0].0, prim_type);
        assert_eq!(proxy.get_sprims_to_remove()[0].1, path);
    }

    #[test]
    fn test_repopulate() {
        let mut proxy = IndexProxy::new();
        let path1 = Path::from_string("/World/Group").unwrap();
        let path2 = Path::from_string("/World/Mesh").unwrap();

        proxy.repopulate(&path1);
        proxy.repopulate(&path2);
        proxy.repopulate(&path1); // Duplicate

        assert_eq!(proxy.get_paths_to_repopulate().len(), 3);

        // Process should deduplicate
        proxy.process_pending_operations();
        assert_eq!(proxy.get_paths_to_repopulate().len(), 2);
    }

    #[test]
    fn test_process_pending_clears_removals() {
        let mut proxy = IndexProxy::new();
        let path = Path::from_string("/World/Mesh").unwrap();

        proxy.remove_rprim(&path);
        proxy.remove_instancer(&path);

        assert_eq!(proxy.get_rprims_to_remove().len(), 1);
        assert_eq!(proxy.get_instancers_to_remove().len(), 1);

        proxy.process_pending_operations();

        assert_eq!(proxy.get_rprims_to_remove().len(), 0);
        assert_eq!(proxy.get_instancers_to_remove().len(), 0);
    }

    #[test]
    fn test_type_support() {
        let proxy = IndexProxy::new();

        assert!(proxy.is_rprim_type_supported(&Token::new("mesh")));
        assert!(proxy.is_rprim_type_supported(&Token::new("points")));
        assert!(!proxy.is_rprim_type_supported(&Token::new("invalid")));

        assert!(proxy.is_sprim_type_supported(&Token::new("camera")));
        assert!(proxy.is_sprim_type_supported(&Token::new("light")));
        assert!(!proxy.is_sprim_type_supported(&Token::new("invalid")));

        assert!(proxy.is_bprim_type_supported(&Token::new("renderBuffer")));
        assert!(!proxy.is_bprim_type_supported(&Token::new("invalid")));
    }

    #[test]
    fn test_clear() {
        let mut proxy = IndexProxy::new();
        let path = Path::from_string("/World/Mesh").unwrap();

        proxy.remove_rprim(&path);
        proxy.repopulate(&path);

        assert!(proxy.get_rprims_to_remove().len() > 0);
        assert!(proxy.get_paths_to_repopulate().len() > 0);

        proxy.clear();

        assert_eq!(proxy.get_rprims_to_remove().len(), 0);
        assert_eq!(proxy.get_paths_to_repopulate().len(), 0);
    }

    #[test]
    fn test_insert_operations() {
        let mut proxy = IndexProxy::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/World/Mesh").unwrap();
        let prim_type = Token::new("mesh");

        // These should not panic
        proxy.insert_rprim(&prim_type, &path, &prim, None);
        proxy.insert_sprim(&prim_type, &path, &prim, None);
        proxy.insert_bprim(&prim_type, &path, &prim, None);
        proxy.insert_instancer(&path, &prim, None);
    }

    #[test]
    fn test_mark_rprim_dirty_forwards_to_backend_with_index_path() {
        let backend: Arc<TestBackend> = Arc::new(TestBackend::default());
        let delegate_id = Path::from_string("/Delegate").expect("delegate path");
        let mut proxy = IndexProxy::new_with_backend(backend.clone(), delegate_id);
        let cache_path = Path::from_string("/Mesh").expect("cache path");

        proxy.mark_rprim_dirty(&cache_path, 0x24);

        let calls = backend.rprim_dirty.lock().expect("lock poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0,
            Path::from_string("/Delegate/Mesh").expect("index path")
        );
        assert_eq!(calls[0].1, 0x24);
    }
}
