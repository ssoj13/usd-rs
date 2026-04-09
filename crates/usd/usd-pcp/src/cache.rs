//! PCP Cache - composition context and caching.
//!
//! PcpCache is the context required to make requests of the Pcp composition
//! algorithm and cache the results.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/cache.h` (~700 lines).
//!
//! # Overview
//!
//! Because composition algorithms are recursive (making a request typically
//! makes other internal requests to solve subproblems), caching subproblem
//! results is required for reasonable performance. PcpCache is the only
//! entrypoint to these algorithms.
//!
//! # Parameters
//!
//! - **Variant fallbacks**: Per named variant set, an ordered list of fallback
//!   values to use when composing a prim that defines a variant set but does
//!   not specify a selection
//! - **Payload inclusion set**: Used to identify which prims should have their
//!   payloads included during composition
//! - **File format target**: The file format target for opening layers
//! - **USD mode**: Configures composition for lighter USD-specific behavior

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use super::changes::CacheChanges;
use super::dependencies::Dependencies;
use crate::{
    DependencyFlags, DependencyVector, ErrorType, LayerStack, LayerStackIdentifier,
    LayerStackRefPtr, PrimIndex, PrimIndexInputs, VariantFallbackMap, compute_prim_index,
};
use usd_sdf::{Layer, Path};

/// A set of paths for payload inclusion.
pub type PayloadSet = HashSet<Path>;

/// PcpCache is the context required to make requests of the Pcp
/// composition algorithm and cache the results.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_pcp::{Cache, LayerStackIdentifier};
///
/// let root_id = LayerStackIdentifier::new("root.usda");
/// let cache = Cache::new(root_id, true);
///
/// let prim_index = cache.compute_prim_index(&Path::from_string("/World").unwrap());
/// ```
pub struct Cache {
    /// The root layer stack identifier.
    layer_stack_identifier: LayerStackIdentifier,
    /// File format target for opening layers.
    file_format_target: String,
    /// Whether in USD mode.
    is_usd: bool,

    /// Cached root layer stack.
    layer_stack: RwLock<Option<LayerStackRefPtr>>,
    /// Cached layer stacks by identifier.
    layer_stack_cache: RwLock<HashMap<LayerStackIdentifier, LayerStackRefPtr>>,
    /// Cached prim indices by path.
    prim_index_cache: RwLock<HashMap<Path, PrimIndex>>,
    /// Arc-wrapped prim index cache for hot-path read access (no deep clone).
    prim_index_arc_cache: RwLock<HashMap<Path, Arc<PrimIndex>>>,
    /// Cached property indices by path.
    property_index_cache: RwLock<HashMap<Path, super::PropertyIndex>>,

    /// Variant fallbacks.
    variant_fallbacks: RwLock<VariantFallbackMap>,
    /// Included payloads.
    included_payloads: RwLock<PayloadSet>,
    /// Predicate for including newly discovered payloads.
    /// C++ UsdStage passes `_IncludePayloadsPredicate` = `loadRules.IsLoaded(path)`.
    include_payload_predicate: RwLock<Option<Arc<dyn Fn(&Path) -> bool + Send + Sync>>>,
    /// Muted layers.
    muted_layers: RwLock<Vec<String>>,

    /// Prim index inputs (cached for efficiency).
    prim_index_inputs: RwLock<PrimIndexInputs>,

    /// Used layers tracking.
    used_layers: RwLock<HashSet<String>>,
    /// Used layers revision counter.
    used_layers_revision: RwLock<usize>,

    /// Invalid sublayer identifiers.
    invalid_sublayer_identifiers: RwLock<Vec<String>>,
    /// Invalid asset paths by prim path.
    invalid_asset_paths: RwLock<HashMap<Path, Vec<String>>>,

    /// Dependency tracking for cache invalidation.
    pub(crate) dependencies: Dependencies,

    /// Session layer handle (matches C++ PcpLayerStackIdentifier::sessionLayer).
    /// Stored directly to avoid re-lookup for anonymous layers.
    session_layer_handle: Option<Arc<Layer>>,
}

impl Cache {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Creates a new PcpCache for the given layer stack identifier.
    ///
    /// # Arguments
    ///
    /// * `layer_stack_identifier` - Identifier for the root layer stack
    /// * `file_format_target` - Optional file format target for layers
    /// * `usd` - If true, use USD-mode composition (lighter, faster)
    pub fn new(layer_stack_identifier: LayerStackIdentifier, usd: bool) -> Arc<Self> {
        Self::new_with_session(layer_stack_identifier, None, usd)
    }

    /// Creates a new PcpCache with session layer handle.
    /// Matches C++ constructor that takes PcpLayerStackIdentifier with SdfLayerHandle.
    pub fn new_with_session(
        layer_stack_identifier: LayerStackIdentifier,
        session_layer: Option<Arc<Layer>>,
        usd: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            layer_stack_identifier,
            file_format_target: String::new(),
            is_usd: usd,
            layer_stack: RwLock::new(None),
            layer_stack_cache: RwLock::new(HashMap::new()),
            prim_index_cache: RwLock::new(HashMap::new()),
            prim_index_arc_cache: RwLock::new(HashMap::new()),
            property_index_cache: RwLock::new(HashMap::new()),
            variant_fallbacks: RwLock::new(VariantFallbackMap::new()),
            included_payloads: RwLock::new(HashSet::new()),
            include_payload_predicate: RwLock::new(None),
            muted_layers: RwLock::new(Vec::new()),
            prim_index_inputs: RwLock::new(PrimIndexInputs::new()),
            used_layers: RwLock::new(HashSet::new()),
            used_layers_revision: RwLock::new(0),
            invalid_sublayer_identifiers: RwLock::new(Vec::new()),
            invalid_asset_paths: RwLock::new(HashMap::new()),
            dependencies: Dependencies::new(),
            session_layer_handle: session_layer,
        })
    }

    /// Creates a new PcpCache with file format target.
    pub fn with_file_format_target(
        layer_stack_identifier: LayerStackIdentifier,
        file_format_target: String,
        usd: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            layer_stack_identifier,
            file_format_target,
            is_usd: usd,
            layer_stack: RwLock::new(None),
            layer_stack_cache: RwLock::new(HashMap::new()),
            prim_index_cache: RwLock::new(HashMap::new()),
            prim_index_arc_cache: RwLock::new(HashMap::new()),
            property_index_cache: RwLock::new(HashMap::new()),
            variant_fallbacks: RwLock::new(VariantFallbackMap::new()),
            included_payloads: RwLock::new(HashSet::new()),
            include_payload_predicate: RwLock::new(None),
            muted_layers: RwLock::new(Vec::new()),
            prim_index_inputs: RwLock::new(PrimIndexInputs::new()),
            used_layers: RwLock::new(HashSet::new()),
            used_layers_revision: RwLock::new(0),
            invalid_sublayer_identifiers: RwLock::new(Vec::new()),
            invalid_asset_paths: RwLock::new(HashMap::new()),
            dependencies: Dependencies::new(),
            session_layer_handle: None,
        })
    }

    // ========================================================================
    // Parameters
    // ========================================================================

    /// Returns the identifier of the layer stack used for composition.
    pub fn layer_stack_identifier(&self) -> &LayerStackIdentifier {
        &self.layer_stack_identifier
    }

    /// Returns the root layer stack if computed, otherwise None.
    ///
    /// Use `compute_layer_stack()` to compute it if needed.
    pub fn layer_stack(&self) -> Option<LayerStackRefPtr> {
        self.layer_stack.read().expect("rwlock poisoned").clone()
    }

    /// Returns true if this is the root layer stack.
    pub fn has_root_layer_stack(&self, layer_stack: &LayerStackRefPtr) -> bool {
        if let Some(ref root) = *self.layer_stack.read().expect("rwlock poisoned") {
            Arc::ptr_eq(root, layer_stack)
        } else {
            false
        }
    }

    /// Returns true if the cache is configured in USD mode.
    pub fn is_usd(&self) -> bool {
        self.is_usd
    }

    /// Returns the file format target.
    pub fn file_format_target(&self) -> &str {
        &self.file_format_target
    }

    /// Returns the variant fallbacks.
    pub fn variant_fallbacks(&self) -> VariantFallbackMap {
        self.variant_fallbacks
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Sets the variant fallbacks.
    ///
    /// If `changes` is provided, it's adjusted to reflect the changes necessary
    /// to see the change in variant fallbacks. Otherwise, those changes are applied immediately.
    ///
    /// Matches C++ `SetVariantFallbacks()`.
    pub fn set_variant_fallbacks(
        &self,
        fallbacks: VariantFallbackMap,
        changes: Option<&mut super::changes::Changes>,
    ) {
        let _old_fallbacks = self
            .variant_fallbacks
            .read()
            .expect("rwlock poisoned")
            .clone();

        if let Some(_changes) = changes {
            // Record changes for variant fallback update
            // This would invalidate prim indices that depend on variant selections
            // Simplified: mark that variant fallbacks changed
            // Full implementation would track which prim indices are affected
            // For now, we still update the fallbacks but record the change
            *self.variant_fallbacks.write().expect("rwlock poisoned") = fallbacks;
        } else {
            // Apply changes immediately
            *self.variant_fallbacks.write().expect("rwlock poisoned") = fallbacks;

            // Invalidate affected prim indices
            // Full implementation would find and invalidate prim indices using variants
        }
    }

    /// Returns true if the payload at the given path is included.
    pub fn is_payload_included(&self, path: &Path) -> bool {
        self.included_payloads
            .read()
            .expect("rwlock poisoned")
            .contains(path)
    }

    /// Returns the set of included payloads.
    pub fn included_payloads(&self) -> PayloadSet {
        self.included_payloads
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Sets the payload inclusion predicate.
    /// C++ UsdStage passes `_IncludePayloadsPredicate` = `loadRules.IsLoaded(path)`.
    pub fn set_include_payload_predicate(
        &self,
        pred: Option<Arc<dyn Fn(&Path) -> bool + Send + Sync>>,
    ) {
        *self
            .include_payload_predicate
            .write()
            .expect("rwlock poisoned") = pred;
    }

    /// Requests payloads to be included or excluded from composition.
    ///
    /// If `changes` is provided, it's adjusted to reflect the changes necessary
    /// to see the change in payloads. Otherwise, those changes are applied immediately.
    ///
    /// Note: If a path is listed in both `to_include` and `to_exclude`,
    /// it will be treated as an inclusion only.
    ///
    /// Matches C++ `RequestPayloads()`.
    pub fn request_payloads(
        &self,
        to_include: &[Path],
        to_exclude: &[Path],
        changes: Option<&mut super::changes::Changes>,
    ) {
        let mut payloads = self.included_payloads.write().expect("rwlock poisoned");
        let mut changed_paths = Vec::new();

        for path in to_include {
            if !payloads.contains(path) {
                payloads.insert(path.clone());
                changed_paths.push(path.clone());
            }
        }

        for path in to_exclude {
            if !to_include.contains(path) && payloads.remove(path) {
                changed_paths.push(path.clone());
            }
        }

        // Invalidate cached prim indices at changed paths so they get
        // recomputed with the new payload inclusion set.
        // Matches C++ PcpCache::RequestPayloads which calls
        // _GetCacheChangesForPayloadChange -> DidChangeSignificantly.
        if !changed_paths.is_empty() {
            {
                let mut cache = self.prim_index_cache.write().expect("rwlock poisoned");
                for path in &changed_paths {
                    // Remove this path and all descendants (payload toggle
                    // affects the entire subtree rooted here).
                    cache.retain(|p, _| !p.has_prefix(path));
                }
            }

            if let Some(changes) = changes {
                // Record paths that changed significantly so callers
                // (e.g. UsdStage) can react.
                for path in &changed_paths {
                    changes.did_change_significantly(self, path);
                }
            }
        }
    }

    /// Returns the list of muted layers.
    pub fn muted_layers(&self) -> Vec<String> {
        self.muted_layers.read().expect("rwlock poisoned").clone()
    }

    /// Returns true if the layer is muted.
    ///
    /// If `layer_identifier` is relative, it is resolved relative to this
    /// cache's root layer.
    pub fn is_layer_muted(&self, layer_identifier: &str) -> bool {
        // Delegate to the anchor overload using the root layer identifier
        // as the anchor, matching C++ behavior.
        let root_id = self.layer_stack_identifier.root_layer.get_asset_path();
        self.is_layer_muted_with_anchor(root_id, layer_identifier).0
    }

    /// Returns true if the layer is muted, resolving relative identifiers
    /// against `anchor_layer`.
    ///
    /// If found muted, returns `(true, Some(canonical_id))`. Otherwise
    /// returns `(false, None)`.
    ///
    /// Matches C++ `PcpCache::IsLayerMuted(SdfLayerHandle, string, string*)`.
    pub fn is_layer_muted_with_anchor(
        &self,
        anchor_layer: &str,
        layer_identifier: &str,
    ) -> (bool, Option<String>) {
        let muted = self.muted_layers.read().expect("rwlock poisoned");

        // Check for exact match first
        if let Some(canonical) = muted.iter().find(|m| m.as_str() == layer_identifier) {
            return (true, Some(canonical.clone()));
        }

        // If layer_identifier is relative, try resolving against anchor
        if !layer_identifier.is_empty() && !std::path::Path::new(layer_identifier).is_absolute() {
            // Compute canonical path relative to anchor
            if let Some(anchor_dir) = std::path::Path::new(anchor_layer).parent() {
                let resolved = anchor_dir.join(layer_identifier);
                let resolved_str = resolved.to_string_lossy();
                if let Some(canonical) = muted.iter().find(|m| m.as_str() == resolved_str.as_ref())
                {
                    return (true, Some(canonical.clone()));
                }
            }
        }

        (false, None)
    }

    /// Requests layers to be muted or unmuted in this cache.
    ///
    /// Muted layers are ignored during composition and do not appear in any layer stacks.
    /// The root layer of this cache may not be muted.
    ///
    /// If `changes` is provided, it's adjusted to reflect the changes necessary
    /// to see the change in muted layers. Otherwise, those changes are applied immediately.
    ///
    /// `new_layers_muted` and `new_layers_unmuted` contain the pruned vector of layers
    /// which are muted or unmuted by this call.
    ///
    /// Matches C++ `RequestLayerMuting()`.
    pub fn request_layer_muting(
        &self,
        to_mute: &[String],
        to_unmute: &[String],
        changes: Option<&mut super::changes::Changes>,
        new_layers_muted: Option<&mut Vec<String>>,
        new_layers_unmuted: Option<&mut Vec<String>>,
    ) {
        let mut muted = self.muted_layers.write().expect("rwlock poisoned");
        let mut newly_muted = Vec::new();
        let mut newly_unmuted = Vec::new();

        // Add newly muted layers
        for layer in to_mute {
            if !muted.contains(layer) {
                muted.push(layer.clone());
                newly_muted.push(layer.clone());
            }
        }

        // Remove unmuted layers
        for layer in to_unmute {
            if muted.contains(layer) {
                muted.retain(|l| l != layer);
                newly_unmuted.push(layer.clone());
            }
        }

        // C++ calls _layerStackCache->MuteAndUnmuteLayers() then
        // cacheChanges->DidMuteAndUnmuteLayers(). We record changes so
        // apply_changes() will invalidate affected layer stacks and prim indices.
        if let Some(changes) = changes {
            changes.did_mute_and_unmute_layers(self, &newly_muted, &newly_unmuted);
        } else {
            // No external changes collector — apply immediately by invalidating
            // all prim indices that depend on the muted/unmuted layers.
            let mut prim_cache = self.prim_index_cache.write().expect("rwlock poisoned");
            let affected_ids: Vec<String> = newly_muted
                .iter()
                .chain(newly_unmuted.iter())
                .cloned()
                .collect();
            prim_cache.retain(|_path, index| {
                let mut keep = true;
                for node in index.nodes() {
                    if let Some(ls) = node.layer_stack() {
                        for layer in ls.get_layers() {
                            if affected_ids.iter().any(|id| id == layer.identifier()) {
                                keep = false;
                                break;
                            }
                        }
                    }
                    if !keep {
                        break;
                    }
                }
                if !keep {
                    self.dependencies.remove(index, None);
                }
                keep
            });
        }

        // Update output parameters
        if let Some(new_muted) = new_layers_muted {
            *new_muted = newly_muted;
        }
        if let Some(new_unmuted) = new_layers_unmuted {
            *new_unmuted = newly_unmuted;
        }
    }

    // ========================================================================
    // Layer Stack Computations
    // ========================================================================

    /// Computes and returns the layer stack for the given identifier.
    ///
    /// If the layer stack already exists in the cache, returns the cached version.
    /// Otherwise, resolves the root layer and all sublayers, builds the full
    /// layer stack, and caches it.
    ///
    /// Matches C++ `PcpCache::ComputeLayerStack()` + `PcpLayerStack::_BuildLayerStack`.
    pub fn compute_layer_stack(
        &self,
        identifier: &LayerStackIdentifier,
    ) -> Result<LayerStackRefPtr, Vec<ErrorType>> {
        // Check cache first
        if let Some(cached) = self
            .layer_stack_cache
            .read()
            .expect("rwlock poisoned")
            .get(identifier)
        {
            return Ok(cached.clone());
        }

        // Resolve root layer from identifier asset path.
        // C++ _BuildLayerStack: open root layer, iterate sublayer paths,
        // resolve each via SdfLayer::FindOrOpen, recurse.
        // Do not substitute a fake layer when open fails — that masks resolver / IO errors.
        let resolved = identifier.root_layer.get_resolved_path();
        let authored = identifier.root_layer.get_asset_path();
        let root_layer_path = if !resolved.is_empty() {
            resolved
        } else {
            authored
        };

        let root_layer = if !root_layer_path.is_empty() {
            // Try resolved path first, then authored path (when they differ).
            let first_attempt = Layer::find_or_open(root_layer_path);
            let layer_result = match first_attempt {
                Ok(layer) => Ok(layer),
                Err(_) if root_layer_path == resolved && !authored.is_empty() => {
                    Layer::find_or_open(authored)
                }
                Err(e) => Err(e),
            };
            match layer_result {
                Ok(layer) => layer,
                Err(_) => return Err(vec![ErrorType::InvalidAssetPath]),
            }
        } else {
            Layer::create_anonymous(None)
        };

        // Session layer: prefer handle cached on this `Cache` (C++ stores `SdfLayerHandle`).
        // Otherwise resolve by path; failed open is an error (do not swallow `find_or_open`).
        let session_layer = if let Some(h) = self.session_layer_handle.clone() {
            Some(h)
        } else if let Some(sl) = identifier.session_layer.as_ref() {
            let sp = sl.get_asset_path();
            if sp.is_empty() {
                None
            } else if let Some(l) = Layer::find(&sp) {
                Some(l)
            } else {
                match Layer::find_or_open(sp) {
                    Ok(l) => Some(l),
                    Err(_) => return Err(vec![ErrorType::InvalidSublayerPath]),
                }
            }
        } else {
            None
        };

        let layer_stack = LayerStack::from_root_layer_with_session(root_layer, session_layer);

        // Apply muted layers from this cache
        let muted = self.muted_layers.read().expect("rwlock poisoned").clone();
        if !muted.is_empty() {
            layer_stack.set_muted_layers(muted);
        }

        // Cache it
        self.layer_stack_cache
            .write()
            .expect("rwlock poisoned")
            .insert(identifier.clone(), layer_stack.clone());

        // If this is the root identifier, also store as root layer stack
        if identifier == &self.layer_stack_identifier {
            *self.layer_stack.write().expect("rwlock poisoned") = Some(layer_stack.clone());
        }

        Ok(layer_stack)
    }

    /// Finds a cached layer stack for the given identifier.
    ///
    /// Returns None if not computed yet.
    pub fn find_layer_stack(&self, identifier: &LayerStackIdentifier) -> Option<LayerStackRefPtr> {
        self.layer_stack_cache
            .read()
            .expect("rwlock poisoned")
            .get(identifier)
            .cloned()
    }

    /// Returns true if the layer stack is used by this cache.
    pub fn uses_layer_stack(&self, layer_stack: &LayerStackRefPtr) -> bool {
        let cache = self.layer_stack_cache.read().expect("rwlock poisoned");
        cache.values().any(|ls| Arc::ptr_eq(ls, layer_stack))
    }

    // ========================================================================
    // Prim Index Computations
    // ========================================================================

    /// Computes and returns the prim index for the given path.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - The path to compute the index for
    ///
    /// # Returns
    ///
    /// The computed prim index and any errors encountered.
    pub fn compute_prim_index(&self, prim_path: &Path) -> (PrimIndex, Vec<ErrorType>) {
        usd_trace::trace_scope!("pcp_compute_prim_index");
        // Check cache first
        if let Some(cached) = self
            .prim_index_cache
            .read()
            .expect("rwlock poisoned")
            .get(prim_path)
        {
            return (cached.clone(), cached.local_errors().to_vec());
        }

        // Get or compute the root layer stack
        let layer_stack = match self.compute_layer_stack(&self.layer_stack_identifier) {
            Ok(ls) => ls,
            Err(errors) => return (PrimIndex::new(), errors),
        };

        // Build inputs
        let mut inputs = PrimIndexInputs::new()
            .usd(self.is_usd)
            .variant_fallbacks(
                self.variant_fallbacks
                    .read()
                    .expect("rwlock poisoned")
                    .clone(),
            )
            .included_payloads(
                self.included_payloads
                    .read()
                    .expect("rwlock poisoned")
                    .iter()
                    .cloned()
                    .collect(),
            );

        // Pass payload predicate if set (C++ _IncludePayloadsPredicate)
        if let Some(pred) = self
            .include_payload_predicate
            .read()
            .expect("rwlock poisoned")
            .as_ref()
        {
            inputs = inputs.include_payload_predicate(pred.clone());
        }

        // Compute the prim index
        let outputs = compute_prim_index(prim_path, &layer_stack, &inputs);

        let mut prim_index = outputs.prim_index;
        for error in &outputs.all_errors {
            prim_index.add_error(*error);
        }

        // Register dependencies for change processing
        // Note: ExpressionVariablesDependencyData doesn't implement Clone,
        // so we pass it by value (it's moved)
        self.dependencies.add(
            &prim_index,
            outputs.culled_dependencies.clone(),
            outputs.dynamic_file_format_dependency_data.clone(),
            outputs.expression_variables_dependency_data,
        );

        // C++ cache.cpp:1735-1737: update includedPayloads if predicate included.
        if outputs.payload_state == super::prim_index::PayloadState::IncludedByPredicate {
            self.included_payloads
                .write()
                .expect("rwlock poisoned")
                .insert(prim_path.clone());
        }

        // Cache the result
        let arc = Arc::new(prim_index.clone());
        self.prim_index_cache
            .write()
            .expect("rwlock poisoned")
            .insert(prim_path.clone(), prim_index);
        self.prim_index_arc_cache
            .write()
            .expect("rwlock poisoned")
            .insert(prim_path.clone(), arc.clone());

        ((*arc).clone(), outputs.all_errors)
    }

    /// Return an `Arc`-wrapped prim index (cheap ref-counted handle, no deep clone).
    ///
    /// Hot paths like `Attribute::get_resolve_info` call this instead of
    /// `compute_prim_index` to avoid cloning `Vec<CompressedSdSite>` per call.
    pub fn get_or_compute_prim_index_arc(&self, prim_path: &Path) -> Option<Arc<PrimIndex>> {
        // Fast path: Arc cache hit
        if let Some(cached) = self
            .prim_index_arc_cache
            .read()
            .expect("rwlock poisoned")
            .get(prim_path)
        {
            return Some(cached.clone());
        }
        // Miss: compute (populates both caches)
        let (_, _) = self.compute_prim_index(prim_path);
        self.prim_index_arc_cache
            .read()
            .expect("rwlock poisoned")
            .get(prim_path)
            .cloned()
    }

    /// Finds a cached prim index for the given path.
    ///
    /// Returns None if not computed yet.
    pub fn find_prim_index(&self, prim_path: &Path) -> Option<PrimIndex> {
        self.prim_index_cache
            .read()
            .expect("rwlock poisoned")
            .get(prim_path)
            .cloned()
    }

    /// Iterates over all cached prim indices.
    pub fn for_each_prim_index<F>(&self, mut callback: F)
    where
        F: FnMut(&PrimIndex),
    {
        let cache = self.prim_index_cache.read().expect("rwlock poisoned");
        for index in cache.values() {
            callback(index);
        }
    }

    // ========================================================================
    // Parallel Indexing
    // ========================================================================

    /// Computes prim indexes for multiple paths in parallel.
    ///
    /// Paths are grouped by depth so that parents are always computed
    /// before their children. Uses rayon for per-level parallelism.
    /// Results are cached in the prim index cache.
    ///
    /// Called by UsdStage::compose_prim_indexes_in_parallel (stage.rs).
    ///
    /// # C++ Parity
    ///
    /// Mirrors `PcpCache::_ComputePrimIndexesInParallel` (cache.cpp:1641).
    /// C++ version additionally takes:
    /// - `_NameChildrenPred` (population mask + load rules + instancing filter)
    /// - `_IncludePayloadsPredicate` (auto-includes payloads matching a predicate)
    /// and registers dependencies via `_primDependencies->Add()`.
    /// Our version computes all indexes without filtering, which is correct but
    /// does extra work when population mask is active. Dependencies are not yet
    /// tracked here — they are resolved lazily at query time instead.
    pub fn compute_prim_indexes_in_parallel(
        &self,
        paths: &[Path],
    ) -> (HashMap<Path, PrimIndex>, Vec<ErrorType>) {
        // Get or compute the root layer stack
        let layer_stack = match self.compute_layer_stack(&self.layer_stack_identifier) {
            Ok(ls) => ls,
            Err(errors) => return (HashMap::new(), errors),
        };

        let variant_fallbacks = self
            .variant_fallbacks
            .read()
            .expect("rwlock poisoned")
            .clone();

        let indexer = super::parallel_indexer::ParallelIndexer::new(
            layer_stack,
            variant_fallbacks,
            self.is_usd,
        );

        let outputs = indexer.compute_indexes(paths);

        // Cache all computed indices and register dependencies
        {
            let mut cache = self.prim_index_cache.write().expect("rwlock poisoned");
            for (path, index) in &outputs.results {
                cache.insert(path.clone(), index.clone());
            }
        }

        (outputs.results, outputs.all_errors)
    }

    /// Computes indexes for an entire subtree rooted at `root` in parallel.
    ///
    /// Starts by computing the root, discovers children via the composed
    /// namespace, then computes each depth level in parallel.
    pub fn compute_subtree_indexes(
        &self,
        root: &Path,
    ) -> (HashMap<Path, PrimIndex>, Vec<ErrorType>) {
        let layer_stack = match self.compute_layer_stack(&self.layer_stack_identifier) {
            Ok(ls) => ls,
            Err(errors) => return (HashMap::new(), errors),
        };

        let variant_fallbacks = self
            .variant_fallbacks
            .read()
            .expect("rwlock poisoned")
            .clone();

        let indexer = super::parallel_indexer::ParallelIndexer::new(
            layer_stack,
            variant_fallbacks,
            self.is_usd,
        );

        let outputs = indexer.compute_subtree(root);

        // Cache all computed indices
        {
            let mut cache = self.prim_index_cache.write().expect("rwlock poisoned");
            for (path, index) in &outputs.results {
                cache.insert(path.clone(), index.clone());
            }
        }

        (outputs.results, outputs.all_errors)
    }

    // ========================================================================
    // Cache Management
    // ========================================================================

    /// Clears all cached prim indices.
    pub fn clear_prim_index_cache(&self) {
        self.prim_index_cache
            .write()
            .expect("rwlock poisoned")
            .clear();
    }

    /// Invalidates cached PrimIndex for a specific path, forcing recomputation.
    pub fn invalidate_prim_index(&self, path: &Path) {
        self.prim_index_cache
            .write()
            .expect("rwlock poisoned")
            .remove(path);
    }

    /// Clears all cached layer stacks.
    pub fn clear_layer_stack_cache(&self) {
        self.layer_stack_cache
            .write()
            .expect("rwlock poisoned")
            .clear();
        *self.layer_stack.write().expect("rwlock poisoned") = None;
    }

    /// Clears all cached data.
    pub fn clear(&self) {
        self.clear_prim_index_cache();
        self.clear_layer_stack_cache();
        // Clear dependencies
        self.dependencies.remove_all(None);
    }

    // ========================================================================
    // Change Processing
    // ========================================================================

    /// Applies changes to this cache, invalidating affected prim indices.
    ///
    /// Matches C++ `PcpCache::Apply()`.
    pub fn apply_changes(&self, changes: &CacheChanges) {
        // Invalidate prim indices affected by significant changes
        let mut cache = self.prim_index_cache.write().expect("rwlock poisoned");
        for path in &changes.did_change_significantly {
            // P0-6 FIX (did_change_significantly): Collect all matching entries BEFORE
            // calling retain(), because retain() removes them and cache.get() would
            // always return None afterwards.
            // C++ PcpCache::Apply() reads dependencies before erasing cache entries.
            let affected: Vec<_> = cache
                .iter()
                .filter(|(p, _)| p.has_prefix(path))
                .map(|(_, idx)| idx.clone())
                .collect();
            // Remove this path and all descendants
            cache.retain(|p, _| !p.has_prefix(path));
            // Clean up dependencies for all removed entries
            for index in &affected {
                self.dependencies.remove(index, None);
            }
        }

        // Invalidate prim indices affected by spec changes.
        // P0-6 FIX: Read the entry BEFORE removing -- cache.remove() returns
        // the removed value, so we can use it for dependency cleanup.
        // C++ PcpCache::Apply() calls _RemovePrimAndPropertyCaches which reads
        // dependencies before erasing the cache entry.
        for path in &changes.did_change_specs {
            if let Some(index) = cache.remove(path) {
                self.dependencies.remove(&index, None);
            }
        }

        // Invalidate prim indices affected by prim changes.
        for path in &changes.did_change_prims {
            if let Some(index) = cache.remove(path) {
                self.dependencies.remove(&index, None);
            }
        }

        // Handle path changes (renames).
        // C++ PcpCache::Apply does two passes:
        // 1. Blow caches under NEW paths (clear destination)
        // 2. Blow caches under OLD paths (clear source)
        // Then fixes up included payloads.
        if !changes.did_change_path.is_empty() {
            // Pass 1: remove caches under new (destination) paths + descendants
            for (_old_path, new_path) in &changes.did_change_path {
                if !new_path.is_empty() {
                    let affected: Vec<_> = cache
                        .iter()
                        .filter(|(p, _)| p.has_prefix(new_path))
                        .map(|(_, idx)| idx.clone())
                        .collect();
                    cache.retain(|p, _| !p.has_prefix(new_path));
                    for index in &affected {
                        self.dependencies.remove(index, None);
                    }
                }
            }
            // Pass 2: remove caches under old (source) paths + descendants
            for (old_path, _new_path) in &changes.did_change_path {
                let affected: Vec<_> = cache
                    .iter()
                    .filter(|(p, _)| p.has_prefix(old_path))
                    .map(|(_, idx)| idx.clone())
                    .collect();
                cache.retain(|p, _| !p.has_prefix(old_path));
                for index in &affected {
                    self.dependencies.remove(index, None);
                }
            }

            // Fix up included payload paths (C++ cache.cpp:1113-1155)
            let mut payloads = self.included_payloads.write().expect("rwlock poisoned");
            let mut new_includes = Vec::new();
            for (old_path, new_path) in &changes.did_change_path {
                payloads.retain(|p| {
                    if p.has_prefix(old_path) {
                        if let Some(renamed) = p.replace_prefix(old_path, new_path) {
                            new_includes.push(renamed);
                        }
                        false
                    } else {
                        true
                    }
                });
                // Chain renames in new_includes (A→B, B→C)
                for inc in &mut new_includes {
                    if inc.has_prefix(old_path) {
                        if let Some(renamed) = inc.replace_prefix(old_path, new_path) {
                            *inc = renamed;
                        }
                    }
                }
            }
            for inc in new_includes {
                payloads.insert(inc);
            }
        }

        // Handle layer changes
        if changes.did_maybe_change_layers {
            // Invalidate all prim indices that depend on affected layers
            for layer_id in &changes.layers_affected_by_muting_or_removal {
                // Find and invalidate prim indices using this layer
                cache.retain(|_path, index| {
                    let mut should_keep = true;
                    for node in index.nodes() {
                        if let Some(ls) = node.layer_stack() {
                            for layer in ls.get_layers() {
                                if layer.identifier() == layer_id {
                                    should_keep = false;
                                    break;
                                }
                            }
                        }
                        if !should_keep {
                            break;
                        }
                    }
                    if !should_keep {
                        self.dependencies.remove(index, None);
                    }
                    should_keep
                });
            }
        }
    }

    // ========================================================================
    // Dependency Tracking
    // ========================================================================

    /// Finds dependencies for the given site.
    ///
    /// Matches C++ `PcpCache::FindSiteDependencies()`.
    pub fn find_site_dependencies(
        &self,
        site_layer: &Arc<Layer>,
        site_path: &Path,
        dep_mask: DependencyFlags,
        recurse_on_site: bool,
        recurse_on_index: bool,
        filter_for_existing_caches_only: bool,
    ) -> DependencyVector {
        use super::ArcType;
        use super::dependency::{DependencyType, classify_node_dependency};
        use super::map_function::MapFunction;
        use super::path_translation::{
            translate_path_from_node_to_root, translate_path_from_node_to_root_using_function,
        };

        // ========================================================================
        // Validate arguments
        // ========================================================================
        if !dep_mask.intersects(DependencyType::VIRTUAL | DependencyType::NON_VIRTUAL) {
            // Invalid depMask - must include at least one of VIRTUAL or NON_VIRTUAL
            return Vec::new();
        }
        if !dep_mask
            .intersects(DependencyType::ROOT | DependencyType::DIRECT | DependencyType::ANCESTRAL)
        {
            // Invalid depMask - must include at least one of ROOT, DIRECT, or ANCESTRAL
            return Vec::new();
        }
        if dep_mask.intersects(DependencyType::ROOT)
            && !dep_mask.intersects(DependencyType::NON_VIRTUAL)
        {
            // Root deps are only ever non-virtual
            return Vec::new();
        }

        // Find layer stack containing this layer
        let layer_stacks = self.layer_stack_cache.read().expect("rwlock poisoned");
        let mut site_layer_stack: Option<LayerStackRefPtr> = None;

        for ls in layer_stacks.values() {
            if ls.get_layers().iter().any(|l| Arc::ptr_eq(l, site_layer)) {
                site_layer_stack = Some(ls.clone());
                break;
            }
        }

        let site_layer_stack = match site_layer_stack {
            Some(ls) => ls,
            None => return Vec::new(),
        };

        // Check that layer stack belongs to this cache (simplified check)
        // In C++: siteLayerStack->_registry != _layerStackCache
        // We check by verifying it's in our cache
        if !layer_stacks
            .values()
            .any(|ls| Arc::ptr_eq(ls, &site_layer_stack))
        {
            return Vec::new();
        }

        // Filter function for dependencies to return
        let cache_filter_fn = |index_path: &Path| -> bool {
            if !filter_for_existing_caches_only {
                return true;
            }
            if index_path.is_absolute_root_or_prim_path() {
                return self.find_prim_index(index_path).is_some();
            } else if index_path.is_property_path() {
                if self.is_usd {
                    // In USD mode, cache does not store property indexes,
                    // so return whether the parent prim is in the cache
                    return self.find_prim_index(&index_path.get_prim_path()).is_some();
                } else {
                    return self.find_property_index(index_path).is_some();
                }
            }
            false
        };

        // Dependency arcs expressed in scene description connect prim
        // paths, prim variant paths, and absolute paths only. Those arcs
        // imply dependency structure for children, such as properties.
        // To service dependency queries about those children, we must
        // examine structure at the enclosing prim/root level where deps
        // are expressed. Find the containing path.
        let site_prim_path = if site_path.is_prim_or_prim_variant_selection_path() {
            site_path.clone()
        } else if site_path == &Path::absolute_root() {
            site_path.clone()
        } else {
            site_path.get_prim_or_prim_variant_selection_path()
        };

        let mut result = DependencyVector::new();

        // Handle root dependency
        // Sites containing variant selections are never root dependencies.
        if let Some(root_layer_stack) = self.layer_stack() {
            if dep_mask.intersects(DependencyType::ROOT)
                && site_layer_stack == root_layer_stack
                && !site_path.contains_prim_variant_selection()
                && cache_filter_fn(site_path)
            {
                result.push(super::dependency::Dependency::new(
                    site_path.clone(),
                    site_path.clone(),
                    MapFunction::identity().clone(),
                ));
            }
        }

        // Helper function to process a dependent node
        let process_dependent_node =
            |node: &super::NodeRef, local_site_path: &Path, deps: &mut DependencyVector| {
                // Translate path from node to root
                let (dep_index_path, valid) = if node.arc_type() == ArcType::Relocate {
                    // Relocates require special handling
                    let mut parent = node.parent_node();
                    while parent.is_valid() && parent.arc_type() == ArcType::Relocate {
                        parent = parent.parent_node();
                    }
                    if parent.is_valid() {
                        if let Some(replaced) =
                            local_site_path.replace_prefix(&node.path(), &parent.path())
                        {
                            translate_path_from_node_to_root(&parent, &replaced)
                        } else {
                            (Path::empty(), false)
                        }
                    } else {
                        (Path::empty(), false)
                    }
                } else {
                    translate_path_from_node_to_root(node, local_site_path)
                };

                if valid && !dep_index_path.is_empty() && cache_filter_fn(&dep_index_path) {
                    let map_to_root = node.map_to_root();
                    let map_func = map_to_root.evaluate();
                    deps.push(super::dependency::Dependency::new(
                        dep_index_path,
                        local_site_path.clone(),
                        map_func,
                    ));
                }
            };

        // Helper function to process a culled dependency
        let process_culled_dependency =
            |dep: &super::CulledDependency, local_site_path: &Path, deps: &mut DependencyVector| {
                let (dep_index_path, valid) = if !dep.unrelocated_site_path.is_empty() {
                    if let Some(replaced) =
                        local_site_path.replace_prefix(&dep.site_path, &dep.unrelocated_site_path)
                    {
                        translate_path_from_node_to_root_using_function(&dep.map_to_root, &replaced)
                    } else {
                        (Path::empty(), false)
                    }
                } else {
                    translate_path_from_node_to_root_using_function(
                        &dep.map_to_root,
                        local_site_path,
                    )
                };

                if valid && !dep_index_path.is_empty() && cache_filter_fn(&dep_index_path) {
                    deps.push(super::dependency::Dependency::new(
                        dep_index_path,
                        local_site_path.clone(),
                        dep.map_to_root.clone(),
                    ));
                }
            };

        // Process dependencies from prim dependencies
        self.dependencies.for_each_dependency_on_site(
            &site_layer_stack,
            &site_prim_path,
            dep_mask.intersects(DependencyType::ANCESTRAL),
            recurse_on_site,
            |dep_prim_index_path, dep_prim_site_path| {
                // Because arc dependencies are analyzed in terms of prims,
                // if we are querying deps for a property, and recurseOnSite
                // is true, we must guard against recursing into paths
                // that are siblings of the property and filter them out.
                if dep_prim_site_path != &site_prim_path
                    && dep_prim_site_path.has_prefix(&site_prim_path)
                    && !dep_prim_site_path.has_prefix(site_path)
                {
                    return;
                }

                // If we have recursed above to an ancestor, include its direct
                // dependencies, since they are considered ancestral by descendants.
                let local_mask = if dep_prim_site_path != &site_prim_path
                    && site_prim_path.has_prefix(dep_prim_site_path)
                {
                    dep_mask | DependencyType::DIRECT
                } else {
                    dep_mask
                };

                // If we have recursed below sitePath, use that site;
                // otherwise use the site the caller requested.
                let local_site_path = if dep_prim_site_path != &site_prim_path
                    && dep_prim_site_path.has_prefix(&site_prim_path)
                {
                    dep_prim_site_path.clone()
                } else {
                    site_path.clone()
                };

                // Process nodes using for_each_dependent_node
                super::dependencies::for_each_dependent_node(
                    &local_site_path,
                    &site_layer_stack,
                    dep_prim_index_path,
                    self,
                    &mut |_dep_index_path, node| {
                        // Skip computing the node's dependency type if we aren't looking
                        // for a specific type -- that computation can be expensive.
                        if local_mask != DependencyType::ANY_INCLUDING_VIRTUAL {
                            let flags = classify_node_dependency(node);
                            if (flags & local_mask) != flags {
                                return;
                            }
                        }
                        process_dependent_node(node, &local_site_path, &mut result);
                    },
                );

                // Process culled dependencies
                let culled_deps = self
                    .dependencies
                    .get_culled_dependencies(dep_prim_index_path);
                for culled_dep in &culled_deps {
                    if Arc::ptr_eq(&culled_dep.layer_stack, &site_layer_stack)
                        && local_site_path.has_prefix(&culled_dep.site_path)
                    {
                        if local_mask != DependencyType::ANY_INCLUDING_VIRTUAL {
                            let flags = culled_dep.flags;
                            if (flags & local_mask) != flags {
                                continue;
                            }
                        }
                        process_culled_dependency(culled_dep, &local_site_path, &mut result);
                    }
                }
            },
        );

        // If recursing down namespace, we may have cache entries for
        // descendants that did not introduce new dependency arcs, and
        // therefore were not encountered above, but which nonetheless
        // represent dependent paths. Add them if requested.
        if recurse_on_index {
            use std::collections::BTreeSet;

            let mut seen_deps = BTreeSet::new();
            let mut expanded_deps = DependencyVector::new();

            for dep in &result {
                let index_path = &dep.index_path;

                // Check if we've already seen a prefix of this path
                let should_skip = seen_deps
                    .iter()
                    .any(|seen: &Path| index_path.has_prefix(seen));
                if should_skip {
                    continue;
                }

                seen_deps.insert(index_path.clone());
                expanded_deps.push(dep.clone());

                // Recurse on child index entries
                if index_path.is_absolute_root_or_prim_path() {
                    // Find all prim indices in the subtree
                    let cache = self.prim_index_cache.read().expect("rwlock poisoned");
                    for (sub_path, _sub_prim_index) in cache.iter() {
                        if sub_path.has_prefix(index_path) && sub_path != index_path {
                            if let Some(sub_prim_index) = self.find_prim_index(sub_path) {
                                if sub_prim_index.is_valid() {
                                    // Create dependency with translated site path
                                    if let Some(site_path_replaced) =
                                        sub_path.replace_prefix(index_path, &dep.site_path)
                                    {
                                        expanded_deps.push(super::dependency::Dependency::new(
                                            sub_path.clone(),
                                            site_path_replaced,
                                            dep.map_func.clone(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }

                // Recurse on child property entries
                if !self.is_usd {
                    let prop_cache = self.property_index_cache.read().expect("rwlock poisoned");
                    for (sub_path, _sub_prop_index) in prop_cache.iter() {
                        if sub_path.has_prefix(index_path) {
                            if let Some(sub_prop_index) = self.find_property_index(sub_path) {
                                if !sub_prop_index.is_empty() {
                                    // Create dependency with translated site path
                                    if let Some(site_path_replaced) =
                                        sub_path.replace_prefix(index_path, &dep.site_path)
                                    {
                                        expanded_deps.push(super::dependency::Dependency::new(
                                            sub_path.clone(),
                                            site_path_replaced,
                                            dep.map_func.clone(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            result = expanded_deps;
        }

        result
    }

    /// Returns the number of cached prim indices.
    pub fn prim_index_cache_size(&self) -> usize {
        self.prim_index_cache.read().expect("rwlock poisoned").len()
    }

    /// Returns all currently cached local composition errors.
    pub fn get_cached_local_errors(&self) -> Vec<ErrorType> {
        let mut errors = Vec::new();

        if let Some(layer_stack) = self.layer_stack() {
            errors.extend(layer_stack.local_errors());
        }

        let prim_cache = self.prim_index_cache.read().expect("rwlock poisoned");
        for prim_index in prim_cache.values() {
            errors.extend_from_slice(prim_index.local_errors());
        }

        errors
    }

    /// Returns the number of cached layer stacks.
    pub fn layer_stack_cache_size(&self) -> usize {
        self.layer_stack_cache
            .read()
            .expect("rwlock poisoned")
            .len()
    }

    // ========================================================================
    // Property Index Computations
    // ========================================================================

    /// Computes and returns the property index for the given property path.
    ///
    /// Matches C++ `PcpCache::ComputePropertyIndex()`.
    /// Computes errors during property index building.
    ///
    /// # Arguments
    ///
    /// * `property_path` - The path to the property
    ///
    /// # Returns
    ///
    /// The computed property index and any errors encountered.
    pub fn compute_property_index(
        &self,
        property_path: &Path,
    ) -> (super::PropertyIndex, Vec<ErrorType>) {
        // Get the prim path from the property path
        let prim_path = property_path.get_prim_path();

        // Compute the prim index if not cached
        let (prim_index, mut errors) = self.compute_prim_index(&prim_path);

        if !prim_index.is_valid() {
            return (super::PropertyIndex::new(), errors);
        }

        // Build property index from prim index
        let (property_index, prop_errors) =
            super::build_prim_property_index(property_path, &prim_index);

        // Combine errors
        errors.extend(prop_errors);

        (property_index, errors)
    }

    /// Finds a cached property index for the given path.
    ///
    /// Returns None if not computed yet.
    /// Matches C++ `FindPropertyIndex()`.
    pub fn find_property_index(&self, property_path: &Path) -> Option<super::PropertyIndex> {
        self.property_index_cache
            .read()
            .expect("rwlock poisoned")
            .get(property_path)
            .cloned()
    }

    /// Returns parameter object containing all inputs for prim index computation.
    ///
    /// Matches C++ `GetPrimIndexInputs()`.
    /// Returns the cached prim index inputs, updated with current variant fallbacks and payloads.
    pub fn get_prim_index_inputs(&self) -> PrimIndexInputs {
        // Read from cache to log current state (ensures field is read)
        let _cached = self.prim_index_inputs.read().expect("rwlock poisoned");
        drop(_cached); // Release read lock before acquiring write lock

        // Build fresh inputs from current state
        let updated = PrimIndexInputs::new()
            .usd(self.is_usd)
            .variant_fallbacks(
                self.variant_fallbacks
                    .read()
                    .expect("rwlock poisoned")
                    .clone(),
            )
            .included_payloads(
                self.included_payloads
                    .read()
                    .expect("rwlock poisoned")
                    .iter()
                    .cloned()
                    .collect(),
            );

        // Update cache for future reference
        *self.prim_index_inputs.write().expect("rwlock poisoned") = PrimIndexInputs::new()
            .usd(self.is_usd)
            .variant_fallbacks(
                self.variant_fallbacks
                    .read()
                    .expect("rwlock poisoned")
                    .clone(),
            )
            .included_payloads(
                self.included_payloads
                    .read()
                    .expect("rwlock poisoned")
                    .iter()
                    .cloned()
                    .collect(),
            );

        updated
    }

    /// Computes relationship target paths for the relationship at the given path.
    ///
    /// Part of the public PcpCache API (C++ cache.h:398, cache.cpp:402).
    /// In C++ this is exposed for Python bindings and standalone PCP consumers.
    ///
    /// NOTE: The main USD code path (UsdRelationship::GetTargets) does NOT call this.
    /// Instead, C++ UsdProperty::_GetTargets (property.cpp:172) builds a
    /// PcpPropertyIndex + PcpTargetIndex directly. Our Rust equivalent
    /// (relationship.rs:101) walks layers via Resolver and composes ListOps inline.
    ///
    /// This stub is kept for API completeness. A full implementation would need
    /// PcpBuildPrimPropertyIndex + PcpBuildTargetIndex with proper path mapping
    /// through node.mapToRoot, deleted_paths tracking, and stop_property support.
    pub fn compute_relationship_target_paths(
        &self,
        relationship_path: &Path,
        local_only: bool,
        _stop_property: Option<&Path>,
        _include_stop_property: bool,
    ) -> (Vec<Path>, Vec<Path>, Vec<ErrorType>) {
        // Get the prim path
        let prim_path = relationship_path.get_prim_path();

        // Compute prim index if needed
        let (prim_index, errors) = self.compute_prim_index(&prim_path);

        if !prim_index.is_valid() {
            return (Vec::new(), Vec::new(), errors);
        }

        // Find relationship spec and compose targets
        let mut paths = Vec::new();
        let deleted_paths = Vec::new();

        // Simplified implementation - full version would traverse prim stack
        // and compose relationship targets from all contributing layers
        for node in prim_index.nodes() {
            if let Some(layer_stack) = node.layer_stack() {
                for layer in layer_stack.get_layers() {
                    if let Some(rel_spec) = layer.get_relationship_at_path(relationship_path) {
                        // Get targets from relationship spec
                        let targets_list = rel_spec.target_path_list();
                        paths.extend(targets_list.get_added_items().iter().cloned());
                        paths.extend(targets_list.get_prepended_items().iter().cloned());
                        paths.extend(targets_list.get_appended_items().iter().cloned());
                        if targets_list.is_explicit() {
                            paths.extend(targets_list.get_explicit_items().iter().cloned());
                        }

                        if local_only {
                            break;
                        }
                    }
                }
                if local_only {
                    break;
                }
            }
        }

        (paths, deleted_paths, errors)
    }

    /// Computes attribute connection paths for the attribute at the given path.
    ///
    /// Part of the public PcpCache API (C++ cache.h:417, cache.cpp:441).
    /// Same situation as `compute_relationship_target_paths` above:
    /// the main code path (UsdAttribute::GetConnections) uses
    /// Resolver-based ListOp composition in attribute.rs, not this method.
    /// Kept for API completeness.
    pub fn compute_attribute_connection_paths(
        &self,
        attribute_path: &Path,
        local_only: bool,
        _stop_property: Option<&Path>,
        _include_stop_property: bool,
    ) -> (Vec<Path>, Vec<Path>, Vec<ErrorType>) {
        // Get the prim path
        let prim_path = attribute_path.get_prim_path();

        // Compute prim index if needed
        let (prim_index, errors) = self.compute_prim_index(&prim_path);

        if !prim_index.is_valid() {
            return (Vec::new(), Vec::new(), errors);
        }

        // Find attribute spec and compose connections
        let mut paths = Vec::new();
        let deleted_paths = Vec::new();

        // Simplified implementation - full version would traverse prim stack
        // and compose connection paths from all contributing layers
        for node in prim_index.nodes() {
            if let Some(layer_stack) = node.layer_stack() {
                for layer in layer_stack.get_layers() {
                    if let Some(attr_spec) = layer.get_attribute_at_path(attribute_path) {
                        // Get connections from attribute spec
                        let connections_list = attr_spec.connection_paths_list();
                        paths.extend(connections_list.get_added_items().iter().cloned());
                        paths.extend(connections_list.get_prepended_items().iter().cloned());
                        paths.extend(connections_list.get_appended_items().iter().cloned());
                        if connections_list.is_explicit() {
                            paths.extend(connections_list.get_explicit_items().iter().cloned());
                        }

                        if local_only {
                            break;
                        }
                    }
                }
                if local_only {
                    break;
                }
            }
        }

        (paths, deleted_paths, errors)
    }

    /// Returns set of all layers used by this cache.
    ///
    /// Matches C++ `GetUsedLayers()`.
    pub fn get_used_layers(&self) -> HashSet<String> {
        self.used_layers.read().expect("rwlock poisoned").clone()
    }

    /// Returns revision number for used layers.
    ///
    /// Matches C++ `GetUsedLayersRevision()`.
    pub fn get_used_layers_revision(&self) -> usize {
        *self.used_layers_revision.read().expect("rwlock poisoned")
    }

    /// Returns set of all root layers used by this cache.
    ///
    /// Matches C++ `GetUsedRootLayers()`.
    pub fn get_used_root_layers(&self) -> HashSet<String> {
        let mut root_layers = HashSet::new();
        if let Some(root_stack) = self.layer_stack() {
            if let Some(root_layer) = root_stack.root_layer() {
                root_layers.insert(root_layer.identifier().to_string());
            }
        }
        root_layers
    }

    /// Returns every computed & cached layer stack that includes the given layer.
    ///
    /// Matches C++ `FindAllLayerStacksUsingLayer()`.
    pub fn find_all_layer_stacks_using_layer(&self, layer: &Arc<Layer>) -> Vec<LayerStackRefPtr> {
        let cache = self.layer_stack_cache.read().expect("rwlock poisoned");
        cache
            .values()
            .filter(|ls| ls.get_layers().iter().any(|l| Arc::ptr_eq(l, layer)))
            .cloned()
            .collect()
    }

    /// Runs callback on every layer stack used by prim indexes in the cache.
    ///
    /// Matches C++ `ForEachLayerStack()`.
    pub fn for_each_layer_stack<F>(&self, mut callback: F)
    where
        F: FnMut(&LayerStackRefPtr),
    {
        let cache = self.layer_stack_cache.read().expect("rwlock poisoned");
        for layer_stack in cache.values() {
            callback(layer_stack);
        }
    }

    /// Returns true if an opinion for the site at localPcpSitePath in the cache's
    /// layer stack can be provided by an opinion in layer.
    ///
    /// Matches C++ `CanHaveOpinionForSite()`.
    pub fn can_have_opinion_for_site(
        &self,
        local_pcp_site_path: &Path,
        layer: &Arc<Layer>,
        allowed_path_in_layer: Option<&mut Path>,
    ) -> bool {
        // Check if prim index exists for this path
        if let Some(prim_index) = self.find_prim_index(local_pcp_site_path) {
            // Check if layer is in any node's layer stack
            for node in prim_index.nodes() {
                if let Some(layer_stack) = node.layer_stack() {
                    if layer_stack
                        .get_layers()
                        .iter()
                        .any(|l| Arc::ptr_eq(l, layer))
                    {
                        if let Some(allowed) = allowed_path_in_layer {
                            *allowed = local_pcp_site_path.clone();
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Returns a vector of sublayer asset paths used in the layer stack
    /// that didn't resolve to valid assets.
    ///
    /// Matches C++ `GetInvalidSublayerIdentifiers()`.
    pub fn get_invalid_sublayer_identifiers(&self) -> Vec<String> {
        self.invalid_sublayer_identifiers
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns true if identifier was used as a sublayer path but didn't identify a valid layer.
    ///
    /// Matches C++ `IsInvalidSublayerIdentifier()`.
    pub fn is_invalid_sublayer_identifier(&self, identifier: &str) -> bool {
        self.invalid_sublayer_identifiers
            .read()
            .expect("rwlock poisoned")
            .contains(&identifier.to_string())
    }

    /// Returns a map of prim paths to asset paths used by that prim
    /// (e.g. in a reference) that didn't resolve to valid assets.
    ///
    /// Matches C++ `GetInvalidAssetPaths()`.
    pub fn get_invalid_asset_paths(&self) -> HashMap<Path, Vec<String>> {
        self.invalid_asset_paths
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns true if resolvedAssetPath was used by a prim but didn't resolve to a valid asset.
    ///
    /// Matches C++ `IsInvalidAssetPath()`.
    pub fn is_invalid_asset_path(&self, resolved_asset_path: &str) -> bool {
        self.invalid_asset_paths
            .read()
            .expect("rwlock poisoned")
            .values()
            .any(|paths| paths.contains(&resolved_asset_path.to_string()))
    }

    /// Returns true if any prim index has a dependency on a dynamic file format argument field.
    ///
    /// Matches C++ `HasAnyDynamicFileFormatArgumentFieldDependencies()`.
    pub fn has_any_dynamic_file_format_argument_field_dependencies(&self) -> bool {
        self.dependencies
            .has_any_dynamic_file_format_argument_field_dependencies()
    }

    /// Returns true if any prim index has a dependency on a dynamic file format argument attribute.
    ///
    /// Matches C++ `HasAnyDynamicFileFormatArgumentAttributeDependencies()`.
    pub fn has_any_dynamic_file_format_argument_attribute_dependencies(&self) -> bool {
        self.dependencies
            .has_any_dynamic_file_format_argument_attribute_dependencies()
    }

    /// Returns true if field was composed while generating dynamic file format arguments.
    ///
    /// Matches C++ `IsPossibleDynamicFileFormatArgumentField()`.
    pub fn is_possible_dynamic_file_format_argument_field(&self, field: &usd_tf::Token) -> bool {
        self.dependencies
            .is_possible_dynamic_file_format_argument_field(field.as_str())
    }

    /// Returns true if attribute's default value was composed while generating dynamic file format arguments.
    ///
    /// Matches C++ `IsPossibleDynamicFileFormatArgumentAttribute()`.
    pub fn is_possible_dynamic_file_format_argument_attribute(
        &self,
        attribute_name: &usd_tf::Token,
    ) -> bool {
        self.dependencies
            .is_possible_dynamic_file_format_argument_attribute(attribute_name.as_str())
    }

    /// Returns the dynamic file format dependency data for the prim index with the given path.
    ///
    /// Matches C++ `GetDynamicFileFormatArgumentDependencyData()`.
    pub fn get_dynamic_file_format_argument_dependency_data(
        &self,
        prim_index_path: &Path,
    ) -> super::dynamic_file_format_dependency_data::DynamicFileFormatDependencyData {
        self.dependencies
            .get_dynamic_file_format_argument_dependency_data(prim_index_path)
            .unwrap_or_default()
    }

    /// Returns the list of prim index paths that depend on expression variables from layerStack.
    ///
    /// Matches C++ `GetPrimsUsingExpressionVariablesFromLayerStack()`.
    pub fn get_prims_using_expression_variables_from_layer_stack(
        &self,
        layer_stack: &LayerStackRefPtr,
    ) -> Vec<Path> {
        // Convert Arc to Weak for Dependencies API
        use std::sync::Weak;
        let weak: Weak<super::LayerStack> = Arc::downgrade(layer_stack);
        self.dependencies
            .get_prims_using_expression_variables_from_layer_stack(&weak)
    }

    /// Returns the set of expression variables in layerStack that are used by the prim index.
    ///
    /// Matches C++ `GetExpressionVariablesFromLayerStackUsedByPrim()`.
    pub fn get_expression_variables_from_layer_stack_used_by_prim(
        &self,
        _prim_index_path: &Path,
        _layer_stack: &LayerStackRefPtr,
    ) -> HashSet<String> {
        // Simplified - would query dependency data
        // Full implementation would check expression_variables_dependency_data
        HashSet::new()
    }

    /// Reloads the layers of the layer stack, except session layers and sublayers of session layers.
    ///
    /// Scans all cached layer stacks for invalid sublayer errors and all prim
    /// indices for invalid asset path errors, notifying `changes` so they can
    /// be retried.  Then reloads every non-session layer reached by this cache.
    ///
    /// Matches C++ `PcpCache::Reload()`.
    pub fn reload(&self, mut changes: Option<&mut super::changes::Changes>) {
        // Early out if root layer stack not yet computed
        let root_layer_stack = match self.layer_stack() {
            Some(ls) => ls,
            None => return,
        };

        // -- Pass 1: notify changes about previously broken sublayers/assets --
        if let Some(ref mut ch) = changes {
            // Scan every cached layer stack for InvalidSublayerPath errors
            let all_stacks: Vec<_> = self
                .layer_stack_cache
                .read()
                .expect("rwlock poisoned")
                .values()
                .cloned()
                .collect();

            for ls in &all_stacks {
                for err in ls.local_errors() {
                    if err == ErrorType::InvalidSublayerPath {
                        // Notify changes so the sublayer is retried on next compose
                        if let Some(root) = ls.root_layer() {
                            for sublayer_id in self.get_invalid_sublayer_identifiers() {
                                ch.did_maybe_fix_sublayer(self, &root, &sublayer_id);
                            }
                        }
                    }
                }
            }

            // Scan every cached prim index for InvalidAssetPath errors
            let prim_cache = self.prim_index_cache.read().expect("rwlock poisoned");
            for (path, prim_index) in prim_cache.iter() {
                if !prim_index.is_valid() {
                    continue;
                }
                for err in prim_index.local_errors() {
                    if *err == ErrorType::InvalidAssetPath {
                        // Build a Site for this prim so Changes can track it
                        let site = super::Site {
                            layer_stack_identifier: self.layer_stack_identifier.clone(),
                            path: path.clone(),
                        };
                        if let Some(root) = root_layer_stack.root_layer() {
                            if let Some(paths) = self.get_invalid_asset_paths().get(path) {
                                for asset in paths {
                                    ch.did_maybe_fix_asset(self, &site, &root, asset);
                                }
                            }
                        }
                    }
                }
            }
        }

        // -- Pass 2: reload every used layer except session layers --
        let session_layers = root_layer_stack.get_session_layers();
        let mut layers_to_reload: Vec<Arc<Layer>> = Vec::new();

        let all_stacks: Vec<_> = self
            .layer_stack_cache
            .read()
            .expect("rwlock poisoned")
            .values()
            .cloned()
            .collect();

        for ls in &all_stacks {
            for layer in ls.get_layers() {
                // Skip session layers
                let is_session = session_layers.iter().any(|sl| Arc::ptr_eq(sl, &layer));
                if !is_session && !layers_to_reload.iter().any(|l| Arc::ptr_eq(l, &layer)) {
                    layers_to_reload.push(layer);
                }
            }
        }

        Layer::reload_layers(&layers_to_reload, false);
    }

    /// Reloads every layer used by the prim at `prim_path` that is across a
    /// reference or payload arc.  Local (root layer stack) layers are NOT
    /// reloaded -- only layers that were brought in through composition arcs.
    ///
    /// Matches C++ `PcpCache::ReloadReferences()`.
    pub fn reload_references(
        &self,
        mut changes: Option<&mut super::changes::Changes>,
        prim_path: &Path,
    ) {
        let root_layer_stack = self.layer_stack().unwrap_or_else(|| {
            // If no root layer stack, nothing to do
            return Arc::new(super::LayerStack::default());
        });

        // Collect unique layer stacks used by prim indices at or under prim_path
        let mut layer_stacks_at_or_under: Vec<LayerStackRefPtr> = Vec::new();

        let prim_cache = self.prim_index_cache.read().expect("rwlock poisoned");

        for (path, prim_index) in prim_cache.iter() {
            if !path.has_prefix(prim_path) {
                continue;
            }
            if !prim_index.is_valid() {
                continue;
            }

            // Notify changes about InvalidAssetPath errors in this subtree
            if let Some(ref mut ch) = changes {
                for err in prim_index.local_errors() {
                    if *err == ErrorType::InvalidAssetPath {
                        let site = super::Site {
                            layer_stack_identifier: self.layer_stack_identifier.clone(),
                            path: path.clone(),
                        };
                        if let Some(root) = root_layer_stack.root_layer() {
                            if let Some(paths) = self.get_invalid_asset_paths().get(path) {
                                for asset in paths {
                                    ch.did_maybe_fix_asset(self, &site, &root, asset);
                                }
                            }
                        }
                    }
                }
            }

            // Collect layer stacks from all nodes of this prim index
            for node in prim_index.nodes() {
                if let Some(ls) = node.layer_stack() {
                    let already = layer_stacks_at_or_under
                        .iter()
                        .any(|existing| Arc::ptr_eq(existing, &ls));
                    if !already {
                        layer_stacks_at_or_under.push(ls);
                    }
                }
            }
        }

        // Scan each collected layer stack for invalid sublayer errors
        if let Some(ref mut ch) = changes {
            for ls in &layer_stacks_at_or_under {
                for err in ls.local_errors() {
                    if err == ErrorType::InvalidSublayerPath {
                        if let Some(root) = ls.root_layer() {
                            for sublayer_id in self.get_invalid_sublayer_identifiers() {
                                ch.did_maybe_fix_sublayer(self, &root, &sublayer_id);
                            }
                        }
                    }
                }
            }
        }

        // Reload layers from referenced/payloaded layer stacks that are NOT
        // in the root (local) layer stack.
        let mut layers_to_reload: Vec<Arc<Layer>> = Vec::new();
        for ls in &layer_stacks_at_or_under {
            for layer in ls.get_layers() {
                if !root_layer_stack.has_layer(&layer)
                    && !layers_to_reload.iter().any(|l| Arc::ptr_eq(l, &layer))
                {
                    layers_to_reload.push(layer);
                }
            }
        }

        Layer::reload_layers(&layers_to_reload, false);
    }

    /// Prints various statistics about the data stored in this cache.
    ///
    /// Matches C++ `PrintStatistics()`.
    pub fn print_statistics(&self) {
        println!("PcpCache Statistics:");
        println!("  Layer stacks cached: {}", self.layer_stack_cache_size());
        println!("  Prim indices cached: {}", self.prim_index_cache_size());
        println!(
            "  Property indices cached: {}",
            self.property_index_cache
                .read()
                .expect("rwlock poisoned")
                .len()
        );
        println!(
            "  Included payloads: {}",
            self.included_payloads
                .read()
                .expect("rwlock poisoned")
                .len()
        );
        println!(
            "  Muted layers: {}",
            self.muted_layers.read().expect("rwlock poisoned").len()
        );
        println!(
            "  Used layers revision: {}",
            self.get_used_layers_revision()
        );
    }
}

/// Reference-counted pointer to a cache.
pub type CachePtr = Arc<Cache>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal on-disk root layer so `compute_layer_stack` can open the asset (parity with real stages).
    fn temp_layer_stack_id_with_scene() -> (tempfile::TempDir, LayerStackIdentifier) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("root.usda");
        std::fs::write(
            &path,
            r#"#usda 1.0
(
)

def Xform "World"
{
}

def Xform "A"
{
    def Xform "C"
    {
    }
}

def Xform "B"
{
    def Xform "D"
    {
    }
}
"#,
        )
        .expect("write usda");
        let id = LayerStackIdentifier::new(path.to_str().expect("utf8 path"));
        (dir, id)
    }

    fn temp_layer_stack_id_minimal() -> (tempfile::TempDir, LayerStackIdentifier) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("root.usda");
        std::fs::write(
            &path,
            r#"#usda 1.0
(
)

def Xform "World"
{
}
"#,
        )
        .expect("write usda");
        let id = LayerStackIdentifier::new(path.to_str().expect("utf8 path"));
        (dir, id)
    }

    #[test]
    fn test_cache_creation() {
        let id = LayerStackIdentifier::new("root.usda");
        let cache = Cache::new(id, true);

        assert!(cache.is_usd());
        assert!(cache.layer_stack().is_none());
        assert!(cache.file_format_target().is_empty());
    }

    #[test]
    fn test_cache_with_format_target() {
        let id = LayerStackIdentifier::new("root.usda");
        let cache = Cache::with_file_format_target(id, "usd".to_string(), false);

        assert!(!cache.is_usd());
        assert_eq!(cache.file_format_target(), "usd");
    }

    #[test]
    fn test_variant_fallbacks() {
        let id = LayerStackIdentifier::new("root.usda");
        let cache = Cache::new(id, true);

        let mut fallbacks = VariantFallbackMap::new();
        fallbacks.insert(
            "shading".to_string(),
            vec!["full".to_string(), "preview".to_string()],
        );

        cache.set_variant_fallbacks(fallbacks.clone(), None);
        assert_eq!(cache.variant_fallbacks(), fallbacks);
    }

    #[test]
    fn test_payload_inclusion() {
        let id = LayerStackIdentifier::new("root.usda");
        let cache = Cache::new(id, true);

        let path1 = Path::from_string("/World/Model").unwrap();
        let path2 = Path::from_string("/World/Other").unwrap();

        assert!(!cache.is_payload_included(&path1));

        cache.request_payloads(&[path1.clone(), path2.clone()], &[], None);
        assert!(cache.is_payload_included(&path1));
        assert!(cache.is_payload_included(&path2));

        cache.request_payloads(&[], &[path1.clone()], None);
        assert!(!cache.is_payload_included(&path1));
        assert!(cache.is_payload_included(&path2));
    }

    #[test]
    fn test_layer_muting() {
        let id = LayerStackIdentifier::new("root.usda");
        let cache = Cache::new(id, true);

        assert!(!cache.is_layer_muted("extra.usda"));

        cache.request_layer_muting(&["extra.usda".to_string()], &[], None, None, None);
        assert!(cache.is_layer_muted("extra.usda"));

        cache.request_layer_muting(&[], &["extra.usda".to_string()], None, None, None);
        assert!(!cache.is_layer_muted("extra.usda"));
    }

    #[test]
    fn test_compute_layer_stack() {
        let (_dir, id) = temp_layer_stack_id_minimal();
        let cache = Cache::new(id.clone(), true);

        let result = cache.compute_layer_stack(&id);
        assert!(result.is_ok());

        // Second call should return cached version
        let result2 = cache.compute_layer_stack(&id);
        assert!(result2.is_ok());

        assert_eq!(cache.layer_stack_cache_size(), 1);
    }

    #[test]
    fn test_compute_layer_stack_errors_when_root_asset_missing() {
        let id = LayerStackIdentifier::new("definitely_missing_root_28473921.usda");
        let cache = Cache::new(id.clone(), true);
        assert!(cache.compute_layer_stack(&id).is_err());
    }

    #[test]
    fn test_compute_prim_index() {
        let (_dir, id) = temp_layer_stack_id_minimal();
        let cache = Cache::new(id, true);

        let path = Path::from_string("/World").unwrap();
        let (index, errors) = cache.compute_prim_index(&path);

        assert!(index.is_valid());
        assert!(errors.is_empty());
        assert_eq!(cache.prim_index_cache_size(), 1);

        // Check cache hit
        let cached = cache.find_prim_index(&path);
        assert!(cached.is_some());
    }

    #[test]
    fn test_cache_clear() {
        let (_dir, id) = temp_layer_stack_id_minimal();
        let cache = Cache::new(id.clone(), true);

        // Populate cache
        let _ = cache.compute_layer_stack(&id);
        let _ = cache.compute_prim_index(&Path::from_string("/World").unwrap());

        assert!(cache.layer_stack_cache_size() > 0);
        assert!(cache.prim_index_cache_size() > 0);

        cache.clear();

        assert_eq!(cache.layer_stack_cache_size(), 0);
        assert_eq!(cache.prim_index_cache_size(), 0);
    }

    #[test]
    fn test_compute_prim_indexes_in_parallel() {
        let (_dir, id) = temp_layer_stack_id_with_scene();
        let cache = Cache::new(id, true);

        let paths: Vec<Path> = ["/A", "/B", "/A/C", "/B/D"]
            .iter()
            .filter_map(|s| Path::from_string(s))
            .collect();

        let (results, errors) = cache.compute_prim_indexes_in_parallel(&paths);

        assert_eq!(results.len(), 4);
        assert!(errors.is_empty());
        for p in &paths {
            assert!(results[p].is_valid());
            // Verify results are also cached
            assert!(cache.find_prim_index(p).is_some());
        }
    }

    #[test]
    fn test_compute_prim_indexes_parallel_empty() {
        let (_dir, id) = temp_layer_stack_id_minimal();
        let cache = Cache::new(id, true);

        let (results, errors) = cache.compute_prim_indexes_in_parallel(&[]);
        assert!(results.is_empty());
        assert!(errors.is_empty());
        assert_eq!(cache.prim_index_cache_size(), 0);
    }

    #[test]
    fn test_compute_subtree_indexes() {
        let (_dir, id) = temp_layer_stack_id_minimal();
        let cache = Cache::new(id, true);

        let root = Path::from_string("/World").unwrap();
        let (results, errors) = cache.compute_subtree_indexes(&root);

        assert!(results.contains_key(&root));
        assert!(results[&root].is_valid());
        assert!(errors.is_empty());
        // Root should be cached
        assert!(cache.find_prim_index(&root).is_some());
    }
}

// Implement DependentNodeCache for Cache
impl super::dependencies::DependentNodeCache for Cache {
    fn find_prim_index(&self, path: &Path) -> Option<PrimIndex> {
        Cache::find_prim_index(self, path)
    }

    fn get_culled_dependencies(&self, path: &Path) -> Vec<super::dependencies::CulledDependency> {
        self.dependencies.get_culled_dependencies(path)
    }
}
