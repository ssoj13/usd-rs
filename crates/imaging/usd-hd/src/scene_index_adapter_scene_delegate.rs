
//! HdSceneIndexAdapterSceneDelegate - Scene delegate backed by scene index.
//!
//! Corresponds to pxr/imaging/hd/sceneIndexAdapterSceneDelegate.h/cpp.
//! This is "back-end" emulation: scenes described via HdSceneIndex become
//! accessible by legacy render delegates through the HdSceneDelegate API.
//!
//! The adapter observes a scene index, tracks prims in a cache, and translates
//! scene index data sources back into delegate method returns (GetTransform,
//! GetMeshTopology, etc.).

// SAFETY: This module uses raw pointers to HdRenderIndex for observer integration.
// All unsafe access is protected by ownership contracts (index outlives adapter).
#![allow(unsafe_code)]

use super::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexObserver,
    HdSceneIndexPrim, RemovedPrimEntry, RenamedPrimEntry,
};
use crate::basis_curves_topology::HdBasisCurvesTopology;
use crate::change_tracker::HdRprimDirtyBits;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocatorSet, HdSampledDataSource,
    HdSampledDataSourceTime, TOK_SCENE_DELEGATE, cast_to_container, cast_to_vector,
    extract_scene_delegate_handle,
};
use crate::dirty_bits_translator::{HD_DIRTY_ALL, HdDirtyBitsTranslator};
use crate::enums::{HdCullStyle, HdInterpolation};
use crate::ext_computation_cpu_callback::HdExtComputationCpuCallbackValue;
use crate::flo_debug::{flo_debug_enabled, summarize_dirtied_entries};
use crate::prim::HdReprSelector;
use crate::prim::mesh::HdMeshTopology;
use crate::render::HdRenderIndex;
use crate::scene_delegate::{
    HdDisplayStyle, HdExtComputationInputDescriptor, HdExtComputationInputDescriptorVector,
    HdExtComputationOutputDescriptor, HdExtComputationOutputDescriptorVector,
    HdExtComputationPrimvarDescriptor, HdExtComputationPrimvarDescriptorVector,
    HdIdVectorSharedPtr, HdPrimvarDescriptor, HdPrimvarDescriptorVector, HdRenderBufferDescriptor,
    HdSyncRequestVector, HdVolumeFieldDescriptor, HdVolumeFieldDescriptorVector,
};
use crate::scene_index::HdSceneIndexPrimView;
use crate::scene_index::{HdSceneIndexNameRegistry, si_ref};
use crate::schema::{
    HdBasisCurvesSchema, HdCameraSchema, HdCategoriesSchema, HdCoordSysBindingSchema,
    HdExtComputationPrimvarsSchema, HdExtComputationSchema, HdExtentSchema,
    HdInstanceCategoriesSchema, HdInstancedBySchema, HdInstancerTopologySchema,
    HdLegacyDisplayStyleSchema, HdLegacyTaskSchema, HdLightSchema, HdMaterialBindingsSchema,
    HdMaterialSchema, HdMeshSchema, HdPrimvarsSchema,
    HdPurposeSchema, HdRenderBufferSchema, HdSubdivisionTagsSchema, HdVisibilitySchema,
    HdVolumeFieldBindingSchema, HdXformSchema, INDEXED_PRIMVAR_VALUE, PRIMVAR_INDICES,
    PRIMVAR_VALUE,
};
use crate::tokens;
use crate::types::HdDirtyBits;
use parking_lot::RwLock;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use usd_gf::{Matrix4d, Range3d};
use usd_px_osd::subdiv_tags::SubdivTags;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::Value;

static DEBUG_GET_TRANSFORM_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_TRANSFORM_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_TRANSFORM_MATRIX_DS_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_TRANSFORM_MATRIX_DS_MISSES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_PRIM_DS_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_PRIM_DS_MISSES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_INPUT_PRIM_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_INPUT_PRIM_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_INPUT_PRIM_SCENE_QUERIES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_INPUT_PRIM_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_GET_TRANSFORM_TOTAL_NS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugTransformStats {
    pub get_transform_calls: usize,
    pub get_transform_cache_hits: usize,
    pub get_transform_matrix_ds_hits: usize,
    pub get_transform_matrix_ds_misses: usize,
    pub get_prim_ds_hits: usize,
    pub get_prim_ds_misses: usize,
    pub get_input_prim_calls: usize,
    pub get_input_prim_cache_hits: usize,
    pub get_input_prim_scene_queries: usize,
    pub get_input_prim_total_ns: u64,
    pub get_transform_total_ns: u64,
}

pub fn reset_debug_transform_stats() {
    DEBUG_GET_TRANSFORM_CALLS.store(0, Ordering::Relaxed);
    DEBUG_GET_TRANSFORM_CACHE_HITS.store(0, Ordering::Relaxed);
    DEBUG_GET_TRANSFORM_MATRIX_DS_HITS.store(0, Ordering::Relaxed);
    DEBUG_GET_TRANSFORM_MATRIX_DS_MISSES.store(0, Ordering::Relaxed);
    DEBUG_GET_PRIM_DS_HITS.store(0, Ordering::Relaxed);
    DEBUG_GET_PRIM_DS_MISSES.store(0, Ordering::Relaxed);
    DEBUG_GET_INPUT_PRIM_CALLS.store(0, Ordering::Relaxed);
    DEBUG_GET_INPUT_PRIM_CACHE_HITS.store(0, Ordering::Relaxed);
    DEBUG_GET_INPUT_PRIM_SCENE_QUERIES.store(0, Ordering::Relaxed);
    DEBUG_GET_INPUT_PRIM_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_GET_TRANSFORM_TOTAL_NS.store(0, Ordering::Relaxed);
}

pub fn read_debug_transform_stats() -> DebugTransformStats {
    DebugTransformStats {
        get_transform_calls: DEBUG_GET_TRANSFORM_CALLS.load(Ordering::Relaxed),
        get_transform_cache_hits: DEBUG_GET_TRANSFORM_CACHE_HITS.load(Ordering::Relaxed),
        get_transform_matrix_ds_hits: DEBUG_GET_TRANSFORM_MATRIX_DS_HITS.load(Ordering::Relaxed),
        get_transform_matrix_ds_misses: DEBUG_GET_TRANSFORM_MATRIX_DS_MISSES.load(Ordering::Relaxed),
        get_prim_ds_hits: DEBUG_GET_PRIM_DS_HITS.load(Ordering::Relaxed),
        get_prim_ds_misses: DEBUG_GET_PRIM_DS_MISSES.load(Ordering::Relaxed),
        get_input_prim_calls: DEBUG_GET_INPUT_PRIM_CALLS.load(Ordering::Relaxed),
        get_input_prim_cache_hits: DEBUG_GET_INPUT_PRIM_CACHE_HITS.load(Ordering::Relaxed),
        get_input_prim_scene_queries: DEBUG_GET_INPUT_PRIM_SCENE_QUERIES.load(Ordering::Relaxed),
        get_input_prim_total_ns: DEBUG_GET_INPUT_PRIM_TOTAL_NS.load(Ordering::Relaxed),
        get_transform_total_ns: DEBUG_GET_TRANSFORM_TOTAL_NS.load(Ordering::Relaxed),
    }
}

fn debug_flo_dirty_enabled() -> bool {
    flo_debug_enabled()
}

#[derive(Clone)]
struct InputPrimCacheEntry {
    generation: u64,
    path: SdfPath,
    prim: HdSceneIndexPrim,
}

thread_local! {
    static INPUT_PRIM_CACHE: RefCell<HashMap<usize, InputPrimCacheEntry>> =
        RefCell::new(HashMap::new());
}

/// Per-prim cache entry tracked by the adapter.
#[allow(dead_code)] // C++ HdSceneIndexAdapterSceneDelegate cache, wiring in progress
struct PrimCacheEntry {
    prim_type: TfToken,
    /// G26: Cached primvar descriptors per interpolation (6 levels).
    primvar_descriptors: Option<Box<[HdPrimvarDescriptorVector; 6]>>,
    /// Match OpenUSD `extCmpPrimvarDescriptors`: cached ext-computation
    /// primvar descriptors per interpolation.
    ext_comp_primvar_descriptors: Option<Box<[HdExtComputationPrimvarDescriptorVector; 6]>>,
    /// G28: Cached locator set for dirty bits translation.
    cached_locator_set: Option<HdDataSourceLocatorSet>,
    /// G28: Cached dirty bits from last sync.
    cached_dirty_bits: HdDirtyBits,
    /// G28: Cached prim type token for type-change detection.
    cached_prim_type: TfToken,
}

/// Prim type classification for render index integration (G29-31).
#[derive(Debug, PartialEq)]
enum PrimKind {
    Rprim,
    Sprim,
    Bprim,
    Instancer,
    GeomSubset,
    Task,
    Unknown,
}

/// Classify prim type into rprim/sprim/bprim for render index integration.
///
/// Matches C++ _PrimAdded logic: check render delegate supported types.
fn classify_prim_type(prim_type: &TfToken, render_index: Option<&HdRenderIndex>) -> PrimKind {
    if prim_type.is_empty() {
        return PrimKind::Unknown;
    }
    if prim_type == "instancer" {
        return PrimKind::Instancer;
    }
    if prim_type == "geomSubset" {
        return PrimKind::GeomSubset;
    }
    if prim_type == "task" {
        return PrimKind::Task;
    }
    if let Some(ri) = render_index {
        if ri.is_rprim_type_supported(prim_type) {
            return PrimKind::Rprim;
        }
        if ri.is_sprim_type_supported(prim_type) {
            return PrimKind::Sprim;
        }
        if ri.is_bprim_type_supported(prim_type) {
            return PrimKind::Bprim;
        }
    }
    PrimKind::Unknown
}

/// Returns true when a primvar locator dirties primvar structure rather than
/// only value payload.
///
/// This mirrors the `_ref` rule used to invalidate cached primvar
/// descriptors: value-only edits (`primvarValue`, `indexedPrimvarValue`,
/// `indices`) preserve descriptor shape, but any other primvar locator may
/// change interpolation/role/indexing and must drop the cache.
fn invalidates_primvar_descriptors(locators: &HdDataSourceLocatorSet) -> bool {
    let primvars_locator = HdPrimvarsSchema::get_default_locator();
    locators.iter().any(|locator| {
        locator.first_element() == primvars_locator.first_element()
            && locator.last_element() != Some(&*PRIMVAR_VALUE)
            && locator.last_element() != Some(&*INDEXED_PRIMVAR_VALUE)
            && locator.last_element() != Some(&*PRIMVAR_INDICES)
    })
}

/// Mark an rprim dirty from a scene-index observer callback.
///
/// `_ref` calls the change tracker directly here, but the current Rust render
/// index drives rprim sync from the `dirty_rprim_ids` acceleration set. Scene
/// index notices therefore must seed that set as well or the dirty rprim may
/// never be visited by `sync_rprims_with_delegate_mut()`.
fn mark_rprim_dirty_from_notice(
    render_index: &mut HdRenderIndex,
    rprim_id: &SdfPath,
    dirty_bits: HdDirtyBits,
) {
    render_index.mark_rprim_dirty(rprim_id, dirty_bits);
}

/// Map interpolation enum to the fixed primvar-descriptor cache slot used by
/// the adapter.
fn primvar_descriptor_cache_index(interpolation: HdInterpolation) -> usize {
    interpolation as usize
}

/// Build the full primvar descriptor cache for a prim in one pass.
///
/// Mirrors `_ref` `_ComputePrimvarDescriptors(...)`: descriptor discovery is
/// independent of the caller's requested interpolation and must populate all
/// buckets at once so later queries see the same cached result.
///
/// The adapter logs slow descriptor builds with per-step timings because
/// `flo.usdz` currently spends most first-load time in this metadata path.
fn compute_primvar_descriptors(
    id: &SdfPath,
    prim_data_source: &HdContainerDataSourceHandle,
) -> Box<[HdPrimvarDescriptorVector; 6]> {
    let total_start = std::time::Instant::now();
    let interpolation_token = TfToken::new("interpolation");
    let role_token = TfToken::new("role");
    let mut descriptors = Box::new(std::array::from_fn(|_| Vec::new()));
    let primvars_container_ms;
    let primvar_names_ms;
    let mut child_lookup_ms = 0.0;
    let mut child_names_ms = 0.0;
    let mut interpolation_ms = 0.0;
    let mut role_ms = 0.0;
    let mut indexed_ms = 0.0;

    let primvars_container = {
        let start = std::time::Instant::now();
        let container = prim_data_source
            .get(&*HdPrimvarsSchema::get_schema_token())
            .and_then(|child| cast_to_container(&child));
        primvars_container_ms = start.elapsed().as_secs_f64() * 1000.0;
        container
    };
    let Some(primvars_container) = primvars_container else {
        return descriptors;
    };

    let primvar_names = {
        let start = std::time::Instant::now();
        let names = primvars_container.get_names();
        primvar_names_ms = start.elapsed().as_secs_f64() * 1000.0;
        names
    };
    let primvar_count = primvar_names.len();

    for name in primvar_names {
        let primvar_container = {
            let start = std::time::Instant::now();
            let container = primvars_container
                .get(&name)
                .and_then(|child| cast_to_container(&child));
            child_lookup_ms += start.elapsed().as_secs_f64() * 1000.0;
            container
        };
        let Some(primvar_container) = primvar_container else {
            continue;
        };

        let child_names = {
            let start = std::time::Instant::now();
            let names = primvar_container.get_names();
            child_names_ms += start.elapsed().as_secs_f64() * 1000.0;
            names
        };

        let (has_role, indexed) = {
            let start = std::time::Instant::now();
            let mut has_role = false;
            let mut has_indexed_value = false;
            let mut has_indices = false;
            for child_name in &child_names {
                if *child_name == role_token {
                    has_role = true;
                } else if *child_name == *INDEXED_PRIMVAR_VALUE {
                    has_indexed_value = true;
                } else if *child_name == *PRIMVAR_INDICES {
                    has_indices = true;
                }
            }
            indexed_ms += start.elapsed().as_secs_f64() * 1000.0;
            (has_role, has_indexed_value && has_indices)
        };

        let interp_token = {
            let start = std::time::Instant::now();
            let token = if let Some(interpolation_ds) = primvar_container.get(&interpolation_token) {
                if let Some(interpolation_sampled) = interpolation_ds.as_sampled() {
                    interpolation_sampled.get_value(0.0).get::<TfToken>().cloned()
                } else {
                    None
                }
            } else {
                None
            };
            interpolation_ms += start.elapsed().as_secs_f64() * 1000.0;
            token
        };
        let Some(interp_token) = interp_token else {
            continue;
        };
        let interp = interpolation_from_token(&interp_token);

        let role = {
            let start = std::time::Instant::now();
            let role = if has_role {
                if let Some(role_ds) = primvar_container.get(&role_token) {
                    if let Some(sampled) = role_ds.as_sampled() {
                        sampled.get_value(0.0).get::<TfToken>().cloned()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            role_ms += start.elapsed().as_secs_f64() * 1000.0;
            role
        };

        descriptors[primvar_descriptor_cache_index(interp)].push(HdPrimvarDescriptor::new(
            name,
            interp,
            role.unwrap_or_default(),
            indexed,
        ));
    }

    let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    if total_ms >= 1.0 {
        log::debug!(
            "HdSceneIndexAdapterSceneDelegate::compute_primvar_descriptors path={} primvars={} total_ms={:.2} primvars_container_ms={:.2} primvar_names_ms={:.2} child_lookup_ms={:.2} child_names_ms={:.2} interpolation_ms={:.2} role_ms={:.2} indexed_ms={:.2}",
            id,
            primvar_count,
            total_ms,
            primvars_container_ms,
            primvar_names_ms,
            child_lookup_ms,
            child_names_ms,
            interpolation_ms,
            role_ms,
            indexed_ms
        );
    }

    descriptors
}

/// Scene delegate that observes a scene index and populates a render index.
///
/// Implements "back-end" emulation: scenes described via HdSceneIndex
/// become accessible to legacy render delegates through HdSceneDelegate API.
///
/// The adapter:
/// 1. Observes PrimsAdded/Removed/Dirtied from the input scene index
/// 2. Maintains a prim cache (path -> type)
/// 3. On delegate queries (GetTransform, etc.), reads data sources from
///    the input scene index and converts them to legacy return types
/// 4. Integrates with RenderIndex for Insert*/Remove*/MarkDirty (G29-31)
///
/// Corresponds to C++ `HdSceneIndexAdapterSceneDelegate`.
pub struct HdSceneIndexAdapterSceneDelegate {
    input_scene_index: HdSceneIndexHandle,
    delegate_id: SdfPath,
    /// Cache of prim types for fast lookup
    prim_cache: RwLock<HashMap<SdfPath, PrimCacheEntry>>,
    /// Generation counter for the per-thread `_GetInputPrim()` cache.
    ///
    /// OpenUSD keeps one last-queried prim per thread. Rust mirrors that with a
    /// thread-local map keyed by adapter address plus a generation counter so
    /// `clear_input_prim_cache()` invalidates all threads without a shared hot lock.
    input_prim_cache_generation: AtomicU64,
    /// Cached underlying legacy delegates discovered via `sceneDelegate`.
    ///
    /// `_ref` builds this list lazily during `Sync()` by scanning the current
    /// prim cache and deduplicating the originating scene delegates. `None`
    /// means the cache is invalid and must be rebuilt from the scene index.
    scene_delegates: RwLock<Option<Vec<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>>>>,
    /// G27: Cache of prim paths known to have geom subset children,
    /// used to avoid redundant child enumeration.
    geom_subset_parents: RwLock<HashSet<SdfPath>>,
    /// Optional render index pointer for Insert*/Remove* integration (G29-31).
    ///
    /// # Safety (P1-4)
    /// - The render index MUST outlive this adapter (ownership contract: adapter is
    ///   owned by the render index, ensuring this holds in practice).
    /// - Access is only from single-threaded observer callbacks (C++ contract).
    /// - NonNull is used to document that the pointer is never null when Some.
    render_index: Option<std::ptr::NonNull<HdRenderIndex>>,
}

// SAFETY: The raw pointer to HdRenderIndex is only accessed from the
// observer methods which are called from a single thread (per C++ contract).
// The render index outlives the adapter (ownership contract).
#[allow(unsafe_code)]
unsafe impl Send for HdSceneIndexAdapterSceneDelegate {}
#[allow(unsafe_code)]
unsafe impl Sync for HdSceneIndexAdapterSceneDelegate {}

impl HdSceneIndexAdapterSceneDelegate {
    /// Create new adapter observing `input_scene_index`.
    ///
    /// Matches C++ constructor: registers in HdSceneIndexNameRegistry,
    /// then traverses all existing prims via HdSceneIndexPrimView and
    /// populates the prim cache + render index.
    ///
    /// NOTE: Observer registration (AddObserver) is deferred to
    /// `register_as_observer()` because Rust ownership prevents
    /// passing `self` during construction. The caller must invoke
    /// `register_as_observer()` after wrapping in Arc<RwLock<>>.
    pub fn new(
        input_scene_index: HdSceneIndexHandle,
        delegate_id: SdfPath,
        render_index: Option<std::ptr::NonNull<HdRenderIndex>>,
    ) -> Self {
        // Step 1: Register in HdSceneIndexNameRegistry (C++ line 162-163)
        let registered_name = format!("delegate adapter: {}", delegate_id.as_str());
        HdSceneIndexNameRegistry::get_instance()
            .register_named_scene_index(&registered_name, &input_scene_index);

        // Step 2: Traverse existing prims and populate cache (C++ line 170-172)
        // Collect all prim paths first, then query each prim individually
        // to avoid holding the scene index read lock during iteration
        // (HdSceneIndexPrimView also acquires read locks internally).
        let mut prim_cache = HashMap::new();
        let mut geom_subset_parents = HashSet::new();
        {
            let view = HdSceneIndexPrimView::new(input_scene_index.clone());
            let prim_paths: Vec<SdfPath> = view.iter().collect();
            for prim_path in &prim_paths {
                let prim = si_ref(&input_scene_index).get_prim(prim_path);
                let prim_type = prim.prim_type.clone();

                // Track geom subset parents (C++ line 337-339)
                if prim_type == "geomSubset" {
                    let parent = prim_path.get_parent_path();
                    if !parent.is_empty() {
                        geom_subset_parents.insert(parent);
                    }
                }

                // Insert into prim cache
                prim_cache.insert(
                    prim_path.clone(),
                    PrimCacheEntry {
                        prim_type: prim_type.clone(),
                        primvar_descriptors: None,
                        ext_comp_primvar_descriptors: None,
                        cached_locator_set: None,
                        cached_dirty_bits: 0,
                        cached_prim_type: TfToken::default(),
                    },
                );

                // Insert into render index (C++ _PrimAdded -> Insert*)
                if let Some(mut ri_ptr) = render_index {
                    // SAFETY: render_index outlives adapter (ownership contract)
                    #[allow(unsafe_code)]
                    let ri = unsafe { ri_ptr.as_mut() };
                    let kind = classify_prim_type(&prim_type, Some(ri));
                    match kind {
                        PrimKind::Rprim => {
                            ri.insert_rprim(&prim_type, &delegate_id, &prim_path);
                        }
                        PrimKind::Sprim => {
                            ri.insert_sprim(&prim_type, &delegate_id, &prim_path);
                        }
                        PrimKind::Bprim => {
                            ri.insert_bprim(&prim_type, &delegate_id, &prim_path);
                        }
                        PrimKind::Instancer => {
                            ri.insert_instancer(&delegate_id, &prim_path);
                        }
                        PrimKind::GeomSubset | PrimKind::Task | PrimKind::Unknown => {}
                    }
                }
            }
        }

        Self {
            input_scene_index,
            delegate_id,
            prim_cache: RwLock::new(prim_cache),
            input_prim_cache_generation: AtomicU64::new(1),
            scene_delegates: RwLock::new(None),
            geom_subset_parents: RwLock::new(geom_subset_parents),
            render_index,
        }
    }

    /// Get the delegate ID prefix.
    pub fn get_delegate_id(&self) -> &SdfPath {
        &self.delegate_id
    }

    fn clear_input_prim_cache(&self) {
        self.input_prim_cache_generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark the cached underlying legacy delegates as invalid.
    ///
    /// `_ref` flips `_sceneDelegatesBuilt=false` whenever prim topology changes
    /// could alter which scene delegates are present in the emulation graph.
    fn invalidate_scene_delegate_cache(&self) {
        *self.scene_delegates.write() = None;
    }

    /// Rebuild the list of originating legacy scene delegates from prim data sources.
    ///
    /// This restores the `_ref` contract where the adapter forwards `Sync()`
    /// and `PostSyncCleanup()` to each distinct underlying delegate referenced
    /// by the emulated scene-index prims.
    fn collect_scene_delegates(&self) -> Vec<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>> {
        if let Some(cached) = self.scene_delegates.read().as_ref() {
            return cached.clone();
        }

        let prim_paths: Vec<SdfPath> = self.prim_cache.read().keys().cloned().collect();
        let mut delegates: Vec<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>> = Vec::new();

        for prim_path in prim_paths {
            let prim = si_ref(&self.input_scene_index).get_prim(&prim_path);
            let Some(data_source) = prim.data_source else {
                continue;
            };
            let Some(scene_delegate_ds) = data_source.get(&TOK_SCENE_DELEGATE) else {
                continue;
            };
            let Some(scene_delegate) = extract_scene_delegate_handle(&scene_delegate_ds) else {
                continue;
            };
            if !delegates
                .iter()
                .any(|existing| Arc::ptr_eq(existing, &scene_delegate))
            {
                delegates.push(scene_delegate);
            }
        }

        *self.scene_delegates.write() = Some(delegates.clone());
        delegates
    }

    /// Handle PrimsDirtied forwarded from stage scene index.
    /// Clears cached prim data so next access reads fresh from terminal SI
    /// (which includes flattening with invalidated xform cache).
    pub fn prims_dirtied_from_stage(&self, entries: &[DirtiedPrimEntry]) {
        // Invalidate per-thread cache entirely (simple and correct).
        self.clear_input_prim_cache();
        let _ = entries;
    }

    /// Get input prim from the scene index with the same per-thread single-entry
    /// cache shape used by `_ref`.
    fn get_input_prim(&self, id: &SdfPath) -> HdSceneIndexPrim {
        let debug_stats = debug_flo_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_GET_INPUT_PRIM_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        let adapter_key = self as *const Self as usize;
        let generation = self.input_prim_cache_generation.load(Ordering::Relaxed);

        let cached = INPUT_PRIM_CACHE.with(|cache| {
            cache
                .borrow()
                .get(&adapter_key)
                .filter(|entry| entry.generation == generation && entry.path == *id)
                .map(|entry| entry.prim.clone())
        });
        if let Some(prim) = cached {
            if debug_stats {
                DEBUG_GET_INPUT_PRIM_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                if let Some(started) = started {
                    DEBUG_GET_INPUT_PRIM_TOTAL_NS
                        .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
            }
            return prim;
        }

        if debug_stats {
            DEBUG_GET_INPUT_PRIM_SCENE_QUERIES.fetch_add(1, Ordering::Relaxed);
        }
        let prim = si_ref(&self.input_scene_index).get_prim(id);
        INPUT_PRIM_CACHE.with(|cache| {
            cache.borrow_mut().insert(
                adapter_key,
                InputPrimCacheEntry {
                    generation,
                    path: id.clone(),
                    prim: prim.clone(),
                },
            );
        });
        if debug_stats {
            if let Some(started) = started {
                DEBUG_GET_INPUT_PRIM_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        prim
    }

    /// Record a prim path as having geom subset children (G27).
    pub fn add_geom_subset_parent(&self, parent_path: &SdfPath) {
        {
            let mut parents = self.geom_subset_parents.write();
            parents.insert(parent_path.clone());
        }
    }

    /// Check if a prim is known to have geom subset children (G27).
    pub fn has_geom_subset_children_hint(&self, path: &SdfPath) -> bool {
        self.geom_subset_parents.read().contains(path)
    }

    /// Helper to get the current prim data source from the input scene index.
    ///
    /// Match OpenUSD: adapter getters read through `_GetInputPrim(id)` rather
    /// than keeping a second retained prim-datasource snapshot in `_primCache`.
    fn get_prim_ds(&self, id: &SdfPath) -> Option<HdContainerDataSourceHandle> {
        if debug_flo_dirty_enabled() {
            DEBUG_GET_PRIM_DS_MISSES.fetch_add(1, Ordering::Relaxed);
        }
        self.get_input_prim(id).data_source
    }

    /// Helper to sample a typed data source, returning times and values.
    fn sample_ds(
        ds: &dyn HdSampledDataSource,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> (Vec<f32>, Vec<Value>) {
        let mut times = Vec::new();
        ds.get_contributing_sample_times(
            start_time as HdSampledDataSourceTime,
            end_time as HdSampledDataSourceTime,
            &mut times,
        );
        // Clamp to max_sample_count
        if times.len() > max_sample_count {
            times.truncate(max_sample_count);
        }
        if times.is_empty() {
            times.push(0.0);
        }
        let values: Vec<Value> = times
            .iter()
            .map(|&t| ds.get_value(t as HdSampledDataSourceTime))
            .collect();
        (times, values)
    }

    // ----------------------------------------------------------------------- //
    // Transform API
    // ----------------------------------------------------------------------- //

    /// Get transform matrix from xform schema.
    ///
    /// Reads HdXformSchema -> matrix data source -> Matrix4d.
    /// Returns identity if schema or data source is absent.
    pub fn get_transform(&self, id: &SdfPath) -> Matrix4d {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static GET_PRIM_NS: AtomicU64 = AtomicU64::new(0);
        static SCHEMA_NS: AtomicU64 = AtomicU64::new(0);
        static TYPED_NS: AtomicU64 = AtomicU64::new(0);

        let debug_stats = debug_flo_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_GET_TRANSFORM_CALLS.fetch_add(1, Ordering::Relaxed);
        }

        let t0 = std::time::Instant::now();
        let prim = self.get_input_prim(id);
        let get_prim_elapsed = t0.elapsed();

        if let Some(data_source) = prim.data_source {
            let t1 = std::time::Instant::now();
            let matrix_ds = HdXformSchema::get_from_parent(&data_source).get_matrix();
            let schema_elapsed = t1.elapsed();

            if let Some(matrix_ds) = matrix_ds {
                let t2 = std::time::Instant::now();
                let value = matrix_ds.get_typed_value(0.0);
                let typed_elapsed = t2.elapsed();

                let n = CALLS.fetch_add(1, Ordering::Relaxed);
                GET_PRIM_NS.fetch_add(get_prim_elapsed.as_nanos() as u64, Ordering::Relaxed);
                SCHEMA_NS.fetch_add(schema_elapsed.as_nanos() as u64, Ordering::Relaxed);
                TYPED_NS.fetch_add(typed_elapsed.as_nanos() as u64, Ordering::Relaxed);

                // Log every 575 calls (one full batch)
                if (n + 1) % 575 == 0 {
                    let gp = GET_PRIM_NS.swap(0, Ordering::Relaxed);
                    let sc = SCHEMA_NS.swap(0, Ordering::Relaxed);
                    let tv = TYPED_NS.swap(0, Ordering::Relaxed);
                    CALLS.store(0, Ordering::Relaxed);
                    log::info!(
                        "[PERF] get_transform batch=575: get_prim={:.1}ms schema={:.1}ms typed_value={:.1}ms total={:.1}ms",
                        gp as f64 / 1_000_000.0,
                        sc as f64 / 1_000_000.0,
                        tv as f64 / 1_000_000.0,
                        (gp + sc + tv) as f64 / 1_000_000.0,
                    );
                }

                if debug_stats {
                    if let Some(started) = started {
                        DEBUG_GET_TRANSFORM_TOTAL_NS
                            .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                    }
                }
                return value;
            }
        }
        if debug_stats {
            if let Some(started) = started {
                DEBUG_GET_TRANSFORM_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        Matrix4d::identity()
    }

    /// Sample transform at multiple time samples (C++ SampleTransform overload 1).
    pub fn sample_transform(
        &self,
        id: &SdfPath,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Matrix4d>,
    ) -> usize {
        self.sample_transform_interval(id, 0.0, 0.0, max_sample_count, out_times, out_values)
    }

    /// Sample transform over interval (C++ SampleTransform overload 2).
    pub fn sample_transform_interval(
        &self,
        id: &SdfPath,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Matrix4d>,
    ) -> usize {
        out_times.clear();
        out_values.clear();
        let prim = self.get_input_prim(id);
        let Some(data_source) = prim.data_source else {
            return 0;
        };
        let xform_schema = HdXformSchema::get_from_parent(&data_source);
        let Some(matrix_ds) = xform_schema.get_matrix() else {
            return 0;
        };
        let (times, _) = Self::sample_ds(&*matrix_ds, start_time, end_time, max_sample_count);
        for &t in &times {
            out_values.push(matrix_ds.get_typed_value(t));
        }
        *out_times = times;
        out_times.len()
    }

    /// Get instancer transform (reads xform schema from instancer prim).
    pub fn get_instancer_transform(&self, instancer_id: &SdfPath) -> Matrix4d {
        self.get_transform(instancer_id)
    }

    /// Sample instancer transform (overload 1).
    pub fn sample_instancer_transform(
        &self,
        instancer_id: &SdfPath,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Matrix4d>,
    ) -> usize {
        self.sample_transform(instancer_id, max_sample_count, out_times, out_values)
    }

    /// Sample instancer transform over interval (overload 2).
    pub fn sample_instancer_transform_interval(
        &self,
        instancer_id: &SdfPath,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Matrix4d>,
    ) -> usize {
        self.sample_transform_interval(
            instancer_id,
            start_time,
            end_time,
            max_sample_count,
            out_times,
            out_values,
        )
    }

    // ----------------------------------------------------------------------- //
    // Rprim API
    // ----------------------------------------------------------------------- //

    /// Get mesh topology from scene index mesh schema.
    ///
    /// Mirrors C++ `HdSceneIndexAdapterSceneDelegate::GetMeshTopology()`:
    /// uses typed schema accessors (`HdMeshSchema`, `HdMeshTopologySchema`)
    /// to read topology data. The typed accessors work for both retained
    /// and attribute-backed data sources via `SampledToTypedAdapter`.
    pub fn get_mesh_topology(&self, id: &SdfPath) -> HdMeshTopology {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return HdMeshTopology::new(),
        };

        // C++: HdMeshSchema meshSchema = HdMeshSchema::GetFromParent(prim.dataSource);
        let mesh_schema = HdMeshSchema::get_from_parent(ds);

        // C++: HdMeshTopologySchema meshTopologySchema = meshSchema.GetTopology();
        let topo_schema = match mesh_schema.get_topology() {
            Some(t) => t,
            None => return HdMeshTopology::new(),
        };

        // C++: HdIntArrayDataSourceHandle faceVertexCountsDataSource =
        //          meshTopologySchema.GetFaceVertexCounts();
        let counts_ds = match topo_schema.get_face_vertex_counts() {
            Some(ds) => ds,
            None => return HdMeshTopology::new(),
        };
        let indices_ds = match topo_schema.get_face_vertex_indices() {
            Some(ds) => ds,
            None => return HdMeshTopology::new(),
        };

        // C++: TfToken scheme = PxOsdOpenSubdivTokens->none;
        let scheme = mesh_schema
            .get_subdivision_scheme()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("none"));

        // C++: VtIntArray holeIndices;
        let holes = topo_schema
            .get_hole_indices()
            .map(|ds| ds.get_typed_value(0.0).to_vec())
            .unwrap_or_default();

        // C++: TfToken orientation = PxOsdOpenSubdivTokens->rightHanded;
        let orientation = topo_schema
            .get_orientation()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("rightHanded"));

        // C++: HdMeshTopology meshTopology(scheme, orientation,
        //          faceVertexCountsDataSource->GetTypedValue(0.0f),
        //          faceVertexIndicesDataSource->GetTypedValue(0.0f),
        //          holeIndices);
        HdMeshTopology::from_full(
            scheme,
            orientation,
            counts_ds.get_typed_value(0.0).to_vec(),
            indices_ds.get_typed_value(0.0).to_vec(),
            holes,
        )
    }

    /// Get basis curves topology from basisCurves schema.
    pub fn get_basis_curves_topology(&self, id: &SdfPath) -> HdBasisCurvesTopology {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => {
                log::warn!("get_basis_curves_topology: no data_source for {}", id);
                return HdBasisCurvesTopology::default();
            }
        };

        // Debug: list available child names on the data source
        let names = ds.get_names();
        log::info!(
            "get_basis_curves_topology: ds names for {}: {:?}",
            id,
            names.iter().map(|n| n.as_str()).collect::<Vec<_>>()
        );

        let bc_schema = HdBasisCurvesSchema::get_from_parent(ds);
        let topo_schema = match bc_schema.get_topology() {
            Some(t) => t,
            None => {
                log::warn!(
                    "get_basis_curves_topology: no topology schema for {} (bc_schema.is_defined={})",
                    id,
                    bc_schema.is_defined()
                );
                return HdBasisCurvesTopology::default();
            }
        };

        let counts_ds = match topo_schema.get_curve_vertex_counts() {
            Some(d) => d,
            None => {
                log::warn!("get_basis_curves_topology: no curveVertexCounts for {}", id);
                return HdBasisCurvesTopology::default();
            }
        };

        let counts: Vec<i32> = counts_ds.get_typed_value(0.0).to_vec();

        let indices: Vec<i32> = topo_schema
            .get_curve_indices()
            .map(|d| d.get_typed_value(0.0).to_vec())
            .unwrap_or_default();

        let basis = topo_schema
            .get_basis()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("bezier"));

        let curve_type = topo_schema
            .get_type()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("linear"));

        let wrap = topo_schema
            .get_wrap()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("nonperiodic"));

        HdBasisCurvesTopology::new(curve_type, basis, wrap, counts, indices)
    }

    /// Get subdivision tags from subdivisionTags schema (G24: GetSubdivTags).
    pub fn get_subdiv_tags(&self, id: &SdfPath) -> SubdivTags {
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => return SubdivTags::default(),
        };

        let subdiv = HdSubdivisionTagsSchema::get_from_parent(&ds);
        if !subdiv.is_defined() {
            return SubdivTags::default();
        }

        let mut tags = SubdivTags::default();

        if let Some(v) = subdiv.get_face_varying_linear_interpolation() {
            tags.set_face_varying_interpolation_rule(v.get_typed_value(0.0));
        }
        if let Some(v) = subdiv.get_interpolate_boundary() {
            tags.set_vertex_interpolation_rule(v.get_typed_value(0.0));
        }
        if let Some(v) = subdiv.get_triangle_subdivision_rule() {
            tags.set_triangle_subdivision(v.get_typed_value(0.0));
        }
        if let Some(v) = subdiv.get_crease_indices() {
            tags.set_crease_indices(v.get_typed_value(0.0).to_vec());
        }
        if let Some(v) = subdiv.get_crease_lengths() {
            tags.set_crease_lengths(v.get_typed_value(0.0).to_vec());
        }
        if let Some(v) = subdiv.get_crease_sharpnesses() {
            tags.set_crease_weights(v.get_typed_value(0.0).to_vec());
        }
        if let Some(v) = subdiv.get_corner_indices() {
            tags.set_corner_indices(v.get_typed_value(0.0).to_vec());
        }
        if let Some(v) = subdiv.get_corner_sharpnesses() {
            tags.set_corner_weights(v.get_typed_value(0.0).to_vec());
        }

        tags
    }

    /// Get visibility from visibility schema.
    pub fn get_visible(&self, id: &SdfPath) -> bool {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return true,
        };

        let vis_schema = HdVisibilitySchema::get_from_parent(ds);
        if !vis_schema.is_defined() {
            return true;
        }

        vis_schema
            .get_visibility()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or(true)
    }

    /// Get double-sided from mesh schema.
    pub fn get_double_sided(&self, id: &SdfPath) -> bool {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return false,
        };

        let mesh_schema = HdMeshSchema::get_from_parent(ds);
        if mesh_schema.is_defined() {
            return mesh_schema
                .get_double_sided()
                .map(|d| d.get_typed_value(0.0))
                .unwrap_or(false);
        }

        if prim.prim_type == "basisCurves" {
            return true;
        }

        false
    }

    /// Get extent from extent schema.
    pub fn get_extent(&self, id: &SdfPath) -> Range3d {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return Range3d::empty(),
        };

        let extent_schema = HdExtentSchema::get_from_parent(ds);
        if !extent_schema.is_defined() {
            return Range3d::empty();
        }

        let min = extent_schema
            .get_min()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or_else(usd_gf::Vec3d::zero);
        let max = extent_schema
            .get_max()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or_else(usd_gf::Vec3d::zero);

        Range3d::new(min, max)
    }

    /// Get display style from legacy display style schema.
    pub fn get_display_style(&self, id: &SdfPath) -> HdDisplayStyle {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return HdDisplayStyle::default(),
        };

        let style_schema = HdLegacyDisplayStyleSchema::get_from_parent(ds);
        if !style_schema.is_defined() {
            return HdDisplayStyle::default();
        }

        let mut result = HdDisplayStyle::default();

        if let Some(v) = style_schema.get_refine_level() {
            result.refine_level = v;
        }
        if let Some(v) = style_schema.get_flat_shading_enabled() {
            result.flat_shading_enabled = v;
        }
        if let Some(v) = style_schema.get_displacement_enabled() {
            result.displacement_enabled = v;
        }
        if let Some(v) = style_schema.get_display_in_overlay() {
            result.display_in_overlay = v;
        }
        if let Some(v) = style_schema.get_occluded_selection_shows_through() {
            result.occluded_selection_shows_through = v;
        }
        if let Some(v) = style_schema.get_points_shading_enabled() {
            result.points_shading_enabled = v;
        }
        if let Some(v) = style_schema.get_material_is_final() {
            result.material_is_final = v;
        }

        result
    }

    /// Get cull style from legacy display style schema.
    pub fn get_cull_style(&self, id: &SdfPath) -> HdCullStyle {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return HdCullStyle::DontCare,
        };

        let style_schema = HdLegacyDisplayStyleSchema::get_from_parent(ds);
        let ct = match style_schema.get_cull_style() {
            Some(t) => t,
            None => return HdCullStyle::DontCare,
        };

        if ct == *tokens::CULL_STYLE_NOTHING {
            HdCullStyle::Nothing
        } else if ct == *tokens::CULL_STYLE_BACK {
            HdCullStyle::Back
        } else if ct == *tokens::CULL_STYLE_FRONT {
            HdCullStyle::Front
        } else if ct == *tokens::CULL_STYLE_BACK_UNLESS_DOUBLE_SIDED {
            HdCullStyle::BackUnlessDoubleSided
        } else if ct == *tokens::CULL_STYLE_FRONT_UNLESS_DOUBLE_SIDED {
            HdCullStyle::FrontUnlessDoubleSided
        } else {
            HdCullStyle::DontCare
        }
    }

    /// Get shading style from legacy display style schema.
    pub fn get_shading_style(&self, id: &SdfPath) -> Value {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return Value::empty(),
        };
        let style_schema = HdLegacyDisplayStyleSchema::get_from_parent(ds);
        if let Some(st) = style_schema.get_shading_style() {
            return Value::from(st);
        }
        Value::empty()
    }

    /// Get repr selector from legacy display style schema.
    ///
    /// C++ reads styleSchema.GetReprSelector() -> token array -> HdReprSelector(ar[0], ar[1], ar[2]).
    pub fn get_repr_selector(&self, id: &SdfPath) -> HdReprSelector {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return HdReprSelector::default(),
        };
        let style_schema = HdLegacyDisplayStyleSchema::get_from_parent(ds);
        if let Some(mut ar) = style_schema.get_repr_selector() {
            // C++ resizes to MAX_TOPOLOGY_REPRS (3), padding with default tokens
            ar.resize(HdReprSelector::MAX_TOPOLOGY_REPRS, TfToken::default());
            return HdReprSelector {
                refined_token: ar[0].clone(),
                unrefined_token: ar[1].clone(),
                points_token: ar[2].clone(),
            };
        }
        HdReprSelector::default()
    }

    /// Get render tag from purpose schema.
    pub fn get_render_tag(&self, id: &SdfPath) -> TfToken {
        let prim = self.get_input_prim(id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return tokens::RENDER_TAG_GEOMETRY.clone(),
        };

        let purpose_schema = HdPurposeSchema::get_from_parent(ds);
        if !purpose_schema.is_defined() {
            return tokens::RENDER_TAG_GEOMETRY.clone();
        }

        purpose_schema
            .get_purpose()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or_else(|| tokens::RENDER_TAG_GEOMETRY.clone())
    }

    /// Get categories (G24: GetCategories).
    ///
    /// Reads HdCategoriesSchema for the prim's category membership.
    pub fn get_categories(&self, id: &SdfPath) -> Vec<TfToken> {
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let cat = HdCategoriesSchema::get_from_parent(&ds);
        if !cat.is_defined() {
            return Vec::new();
        }
        cat.get_included_category_names()
    }

    /// Get volume field descriptors (G24: GetVolumeFieldDescriptors).
    ///
    /// Reads HdVolumeFieldBindingSchema and returns field descriptors.
    pub fn get_volume_field_descriptors(
        &self,
        volume_id: &SdfPath,
    ) -> HdVolumeFieldDescriptorVector {
        let ds = match self.get_prim_ds(volume_id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let vfb = HdVolumeFieldBindingSchema::get_from_parent(&ds);
        if !vfb.is_defined() {
            return Vec::new();
        }

        let mut result = Vec::new();
        for name in vfb.get_volume_field_binding_names() {
            if let Some(path_ds) = vfb.get_volume_field_binding(&name) {
                let field_path: SdfPath = path_ds.get_typed_value(0.0);

                // Query the actual prim type from scene index (C++ parity)
                let field_prim = si_ref(&self.input_scene_index).get_prim(&field_path);
                if field_prim.data_source.is_none() {
                    continue;
                }
                result.push(HdVolumeFieldDescriptor::new(
                    name,
                    field_prim.prim_type,
                    field_path,
                ));
            }
        }
        result
    }

    // ----------------------------------------------------------------------- //
    // Material API
    // ----------------------------------------------------------------------- //

    /// Get material id from materialBindings schema.
    ///
    /// Uses the render delegate's material binding purpose (matching C++).
    pub fn get_material_id(&self, id: &SdfPath) -> Option<SdfPath> {
        let prim = self.get_input_prim(id);
        let ds = prim.data_source.as_ref()?;

        let bindings = HdMaterialBindingsSchema::get_from_parent(ds);
        if !bindings.is_defined() {
            return None;
        }

        // C++: GetRenderIndex().GetRenderDelegate()->GetMaterialBindingPurpose()
        let purpose = if let Some(ri_ptr) = self.render_index {
            let ri = unsafe { ri_ptr.as_ref() };
            let rd = ri.get_render_delegate().read();
            rd.get_material_binding_purpose()
        } else {
            TfToken::default()
        };

        let binding = bindings.get_material_binding(&purpose);
        binding.get_path()
    }

    /// Get material resource (G24: GetMaterialResource).
    ///
    /// Reads HdMaterialSchema, walks the material network graph, and returns
    /// a fully-populated HdMaterialNetworkMap wrapped in a VtValue.
    /// Port of C++ HdSceneIndexAdapterSceneDelegate::GetMaterialResource +
    /// helpers _Walk / _ToMaterialNetworkMap (sceneIndexAdapterSceneDelegate.cpp).
    pub fn get_material_resource(&self, id: &SdfPath) -> Value {
        use crate::material_network::{
            HdMaterialNetworkMap, HdMaterialNetworkV1, HdMaterialNode, HdMaterialRelationship,
        };
        use crate::schema::HdMaterialConnectionSchema;
        use crate::schema::material_network::HdMaterialNetworkSchema;
        use crate::schema::material_node::HdMaterialNodeSchema;
        use std::collections::HashSet;

        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => return Value::empty(),
        };
        let material_schema = HdMaterialSchema::get_from_parent(&ds);
        if !material_schema.is_defined() {
            return Value::empty();
        }

        // Query universal render context (no render delegate access here).
        // C++ uses GetRenderDelegate()->GetMaterialRenderContexts(); we fall
        // back to universal ("") which is tried last by get_material_network().
        let net_container = match material_schema.get_material_network() {
            Some(c) => c,
            None => return Value::empty(),
        };
        let net_schema = HdMaterialNetworkSchema::new(net_container);
        if !net_schema.is_defined() {
            return Value::empty();
        }

        // Retrieve the flat nodes and terminals containers once.
        let nodes_container = net_schema.get_nodes();
        let terminals_container = match net_schema.get_terminals() {
            Some(c) => c,
            None => return Value::empty(),
        };

        let value_token = TfToken::new("value");

        // Reads all parameters from a node's parameter container.
        // Each child of params_container is itself a container {value: DS, ...}.
        let get_params =
            |params_container: HdContainerDataSourceHandle| -> std::collections::BTreeMap<TfToken, Value> {
                let mut params = std::collections::BTreeMap::new();
                for p_name in params_container.get_names() {
                    let Some(p_ds) = params_container.get(&p_name) else {
                        continue;
                    };
                    // Parameter is a container: {value: SampledDS, colorSpace?, typeName?}
                    if let Some(p_cont) = cast_to_container(&p_ds) {
                        if let Some(value_ds) = p_cont.get(&value_token) {
                            if let Some(s) = value_ds.as_sampled() {
                                params.insert(p_name, s.get_value(0.0));
                            }
                        }
                    } else if let Some(s) = p_ds.as_sampled() {
                        // Bare sampled data source (simpler encoding).
                        params.insert(p_name, s.get_value(0.0));
                    }
                }
                params
            };

        // Recursive node walker matching C++ _Walk.
        // Appends relationships (connections) and nodes (post-order) to `net`.
        fn walk(
            node_path: &SdfPath,
            nodes_container: &Option<HdContainerDataSourceHandle>,
            visited: &mut HashSet<SdfPath>,
            net: &mut HdMaterialNetworkV1,
            get_params: &dyn Fn(
                HdContainerDataSourceHandle,
            ) -> std::collections::BTreeMap<TfToken, Value>,
        ) {
            if visited.contains(node_path) {
                return;
            }
            visited.insert(node_path.clone());

            // Nodes are keyed by path-as-token in the schema container.
            let node_name_token = TfToken::new(node_path.as_str());
            let node_schema = {
                let cont = match nodes_container {
                    Some(c) => c,
                    None => return,
                };
                match cont
                    .get(&node_name_token)
                    .and_then(|ds| cast_to_container(&ds))
                {
                    Some(c) => HdMaterialNodeSchema::new(c),
                    None => return,
                }
            };
            if !node_schema.is_defined() {
                return;
            }

            // Read shader identifier.
            let node_id = node_schema
                .get_node_identifier()
                .map(|ds| ds.get_typed_value(0.0))
                .unwrap_or_default();

            // Walk input connections: container {inputName -> vector[ConnectionDS]}.
            if let Some(conn_container) = node_schema.get_input_connections() {
                for conn_name in conn_container.get_names() {
                    let Some(conn_ds) = conn_container.get(&conn_name) else {
                        continue;
                    };
                    // Each input name maps to a vector of connection data sources.
                    if let Some(vec_ds) = cast_to_vector(&conn_ds) {
                        for i in 0..vec_ds.get_num_elements() {
                            let Some(elem) = vec_ds.get_element(i) else {
                                continue;
                            };
                            let Some(elem_cont) = cast_to_container(&elem) else {
                                continue;
                            };
                            let conn_schema = HdMaterialConnectionSchema::new(elem_cont);
                            let upstream_path_tk = conn_schema
                                .get_upstream_node_path()
                                .map(|ds| ds.get_typed_value(0.0))
                                .unwrap_or_default();
                            if upstream_path_tk.is_empty() {
                                continue;
                            }
                            let upstream_output = conn_schema
                                .get_upstream_node_output_name()
                                .map(|ds| ds.get_typed_value(0.0))
                                .unwrap_or_default();

                            let upstream_path =
                                SdfPath::from_string(upstream_path_tk.as_str()).unwrap_or_default();

                            // Recurse into upstream node first (pre-order walk).
                            walk(&upstream_path, nodes_container, visited, net, get_params);

                            // Record the relationship.
                            net.relationships.push(HdMaterialRelationship {
                                input_id: upstream_path.clone(),
                                input_name: upstream_output,
                                output_id: node_path.clone(),
                                output_name: conn_name.clone(),
                            });
                        }
                    }
                }
            }

            // Build the node entry (post-order: upstream nodes already added).
            let parameters = node_schema
                .get_parameters()
                .map(|c| get_params(c))
                .unwrap_or_default();
            net.nodes.push(HdMaterialNode {
                path: node_path.clone(),
                identifier: node_id,
                parameters,
            });
        }

        // Iterate terminals, walk each network, assemble the map.
        let mut mat_map = HdMaterialNetworkMap::default();
        let terminal_names = terminals_container.get_names();

        for terminal_name in terminal_names {
            let Some(term_ds) = terminals_container.get(&terminal_name) else {
                continue;
            };
            let Some(term_cont) = cast_to_container(&term_ds) else {
                continue;
            };
            let term_schema = HdMaterialConnectionSchema::new(term_cont);
            let path_tk = term_schema
                .get_upstream_node_path()
                .map(|ds| ds.get_typed_value(0.0))
                .unwrap_or_default();
            if path_tk.is_empty() {
                continue;
            }
            let terminal_node_path = SdfPath::from_string(path_tk.as_str()).unwrap_or_default();

            mat_map.terminals.push(terminal_node_path.clone());

            let net = mat_map.map.entry(terminal_name).or_default();
            let mut visited: HashSet<SdfPath> = HashSet::new();
            walk(
                &terminal_node_path,
                &nodes_container,
                &mut visited,
                net,
                &get_params,
            );
        }

        Value::from(mat_map)
    }

    /// Get coord sys bindings from coordSysBinding schema.
    ///
    /// Iterates all binding names, gets the path data source for each,
    /// and returns a vector of bound coord system prim paths.
    pub fn get_coord_sys_bindings(&self, id: &SdfPath) -> Option<HdIdVectorSharedPtr> {
        let prim = self.get_input_prim(id);
        let ds = prim.data_source.as_ref()?;

        let coord_sys = HdCoordSysBindingSchema::get_from_parent(ds);
        let names = coord_sys.get_coord_sys_binding_names();
        if names.is_empty() {
            return None;
        }

        let mut id_vec = Vec::new();
        for name in &names {
            if let Some(path_ds) = coord_sys.get_coord_sys_binding(name) {
                id_vec.push(path_ds.get_typed_value(0.0));
            }
        }
        Some(std::sync::Arc::new(id_vec))
    }

    // ----------------------------------------------------------------------- //
    // Primvar API
    // ----------------------------------------------------------------------- //

    /// Get primvar descriptors for a given interpolation.
    pub fn get_primvar_descriptors(
        &self,
        id: &SdfPath,
        interpolation: HdInterpolation,
    ) -> HdPrimvarDescriptorVector {
        if let Some(cached) = self
            .prim_cache
            .read()
            .get(id)
            .and_then(|entry| entry.primvar_descriptors.as_ref())
            .map(|cached| cached[primvar_descriptor_cache_index(interpolation)].clone())
        {
            return cached;
        }

        let prim = self.get_input_prim(id);
        let Some(ds) = prim.data_source else {
            return Vec::new();
        };

        let all = compute_primvar_descriptors(id, &ds);
        let result = all[primvar_descriptor_cache_index(interpolation)].clone();
        if let Some(cache_entry) = self.prim_cache.write().get_mut(id) {
            cache_entry.primvar_descriptors = Some(all);
        }

        result
    }

    /// Get a primvar value by name (generic).
    pub fn get(&self, id: &SdfPath, key: &TfToken) -> Value {
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => return Value::empty(),
        };

        let primvars = HdPrimvarsSchema::get_from_parent(&ds);
        if primvars.is_defined() {
            if let Some(primvar_container) = primvars.get_primvar(key) {
                use crate::schema::PRIMVAR_VALUE;
                if let Some(value_ds) = primvar_container.get(&PRIMVAR_VALUE) {
                    if let Some(sampled) = value_ds.as_sampled() {
                        return sampled.get_value(0.0);
                    }
                }
            }
        }

        Value::empty()
    }

    /// Get indexed primvar value with indices.
    pub fn get_indexed_primvar(
        &self,
        id: &SdfPath,
        key: &TfToken,
        out_indices: &mut Vec<i32>,
    ) -> Value {
        out_indices.clear();
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => return Value::empty(),
        };

        let primvars = HdPrimvarsSchema::get_from_parent(&ds);
        if !primvars.is_defined() {
            return Value::empty();
        }

        let primvar = primvars.get_primvar_schema(key);

        if let Some(value_ds) = primvar.get_indexed_primvar_value() {
            if let Some(sampled) = value_ds.as_sampled() {
                if let Some(indices_ds) = primvar.get_indices() {
                    let arr = indices_ds.get_typed_value(0.0);
                    *out_indices = arr.to_vec();
                }
                return sampled.get_value(0.0);
            }
        }

        Value::empty()
    }

    /// Sample primvar at multiple time samples (G24: SamplePrimvar overload 1).
    pub fn sample_primvar(
        &self,
        id: &SdfPath,
        key: &TfToken,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Value>,
    ) -> usize {
        self.sample_primvar_interval(id, key, 0.0, 0.0, max_sample_count, out_times, out_values)
    }

    /// Sample primvar over interval (G24: SamplePrimvar overload 2).
    pub fn sample_primvar_interval(
        &self,
        id: &SdfPath,
        key: &TfToken,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Value>,
    ) -> usize {
        out_times.clear();
        out_values.clear();
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => {
                out_times.push(0.0);
                out_values.push(Value::empty());
                return 1;
            }
        };

        let primvars = HdPrimvarsSchema::get_from_parent(&ds);
        if !primvars.is_defined() {
            out_times.push(0.0);
            out_values.push(Value::empty());
            return 1;
        }

        if let Some(primvar_container) = primvars.get_primvar(key) {
            use crate::schema::{INDEXED_PRIMVAR_VALUE, PRIMVAR_VALUE};
            // C++ parity: SamplePrimvar without indices output uses GetPrimvarValue()
            // which returns flattened data. Try non-indexed first, then indexed + flatten.
            let value_ds = primvar_container.get(&PRIMVAR_VALUE);
            if let Some(value_ds) = value_ds {
                if let Some(sampled) = value_ds.as_sampled() {
                    let (times, values) =
                        Self::sample_ds(sampled, start_time, end_time, max_sample_count);
                    *out_times = times;
                    *out_values = values;
                    return out_times.len();
                }
            }
            // Indexed primvar: return raw (unflattened) values.
            // Caller must handle index expansion (e.g. faceVarying triangulation).
            // C++ parity: GetPrimvar auto-flattens, but SamplePrimvar does NOT
            // flatten when sampleIndices output is null (returns raw values).
            let indexed_ds = primvar_container.get(&INDEXED_PRIMVAR_VALUE);
            if let Some(indexed_ds) = indexed_ds {
                if let Some(sampled) = indexed_ds.as_sampled() {
                    let (times, values) =
                        Self::sample_ds(sampled, start_time, end_time, max_sample_count);
                    *out_times = times;
                    *out_values = values;
                    return out_times.len();
                }
            }
        }

        out_times.push(0.0);
        out_values.push(Value::empty());
        1
    }

    /// Sample indexed primvar (G24: SampleIndexedPrimvar overload 1).
    pub fn sample_indexed_primvar(
        &self,
        id: &SdfPath,
        key: &TfToken,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Value>,
        out_indices: &mut Vec<Vec<i32>>,
    ) -> usize {
        self.sample_indexed_primvar_interval(
            id,
            key,
            0.0,
            0.0,
            max_sample_count,
            out_times,
            out_values,
            out_indices,
        )
    }

    /// Sample indexed primvar over interval (G24: SampleIndexedPrimvar overload 2).
    pub fn sample_indexed_primvar_interval(
        &self,
        id: &SdfPath,
        key: &TfToken,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Value>,
        out_indices: &mut Vec<Vec<i32>>,
    ) -> usize {
        out_times.clear();
        out_values.clear();
        out_indices.clear();

        let n = self.sample_primvar_interval(
            id,
            key,
            start_time,
            end_time,
            max_sample_count,
            out_times,
            out_values,
        );

        // Read indices for each sample time
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => {
                for _ in 0..n {
                    out_indices.push(Vec::new());
                }
                return n;
            }
        };
        let primvars = HdPrimvarsSchema::get_from_parent(&ds);
        let primvar = primvars.get_primvar_schema(key);
        if let Some(indices_ds) = primvar.get_indices() {
            for &t in out_times.iter() {
                out_indices.push(indices_ds.get_typed_value(t).to_vec());
            }
        } else {
            for _ in 0..n {
                out_indices.push(Vec::new());
            }
        }

        n
    }

    // ----------------------------------------------------------------------- //
    // Instancer API
    // ----------------------------------------------------------------------- //

    /// Get instance categories from instanceCategories schema.
    ///
    /// Iterates vector data source elements, casts each to HdCategoriesSchema,
    /// and collects included category names per instance.
    pub fn get_instance_categories(&self, instancer_id: &SdfPath) -> Vec<Vec<TfToken>> {
        let ds = match self.get_prim_ds(instancer_id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let inst_cat = HdInstanceCategoriesSchema::get_from_parent(&ds);
        if !inst_cat.is_defined() {
            return Vec::new();
        }
        let mut result = Vec::new();
        if let Some(values_ds) = inst_cat.get_categories_values() {
            let n = values_ds.get_num_elements();
            result.reserve(n);
            for i in 0..n {
                if let Some(elem) = values_ds.get_element(i) {
                    // Cast element to container, then wrap as HdCategoriesSchema
                    if let Some(container) = cast_to_container(&elem) {
                        let cat = HdCategoriesSchema::new(container);
                        result.push(cat.get_included_category_names());
                    } else {
                        result.push(Vec::new());
                    }
                } else {
                    result.push(Vec::new());
                }
            }
        }
        result
    }

    /// Get instance indices (G24: GetInstanceIndices).
    ///
    /// Reads from instancerTopology -> instanceIndices container.
    pub fn get_instance_indices(&self, instancer_id: &SdfPath, prototype_id: &SdfPath) -> Vec<i32> {
        let ds = match self.get_prim_ds(instancer_id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let topo = HdInstancerTopologySchema::get_from_parent(&ds);
        if !topo.is_defined() {
            return Vec::new();
        }

        // Read prototypes to find matching index
        let prototypes = match topo.get_prototypes() {
            Some(ds) => ds.get_typed_value(0.0),
            None => return Vec::new(),
        };

        let proto_idx = prototypes.iter().position(|p| p == prototype_id);
        let proto_idx = match proto_idx {
            Some(i) => i,
            None => return Vec::new(),
        };

        // Read instanceIndices container, key by prototype index
        if let Some(indices_container) = topo.get_instance_indices() {
            let key = TfToken::new(&proto_idx.to_string());
            if let Some(child) = indices_container.get(&key) {
                if let Some(sampled) = child.as_sampled() {
                    let val = sampled.get_value(0.0);
                    if let Some(arr) = val.get::<Vec<i32>>() {
                        return arr.clone();
                    }
                }
            }
        }

        Vec::new()
    }

    /// Get instancer id for a prim (G24: GetInstancerId).
    ///
    /// Reads instancedBy schema to find the parent instancer.
    pub fn get_instancer_id(&self, prim_id: &SdfPath) -> SdfPath {
        let ds = match self.get_prim_ds(prim_id) {
            Some(ds) => ds,
            None => return SdfPath::empty(),
        };
        let instanced_by = HdInstancedBySchema::get_from_parent(&ds);
        if !instanced_by.is_defined() {
            return SdfPath::empty();
        }
        if let Some(paths_ds) = instanced_by.get_paths() {
            let paths = paths_ds.get_typed_value(0.0);
            if let Some(first) = paths.first() {
                return first.clone();
            }
        }
        SdfPath::empty()
    }

    /// Get instancer prototypes (G24: GetInstancerPrototypes).
    pub fn get_instancer_prototypes(&self, instancer_id: &SdfPath) -> Vec<SdfPath> {
        let ds = match self.get_prim_ds(instancer_id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let topo = HdInstancerTopologySchema::get_from_parent(&ds);
        if !topo.is_defined() {
            return Vec::new();
        }
        topo.get_prototypes()
            .map(|ds| ds.get_typed_value(0.0).to_vec())
            .unwrap_or_default()
    }

    // ----------------------------------------------------------------------- //
    // RenderBuffer API
    // ----------------------------------------------------------------------- //

    /// Get render buffer descriptor (G24: GetRenderBufferDescriptor).
    pub fn get_render_buffer_descriptor(&self, id: &SdfPath) -> HdRenderBufferDescriptor {
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => return HdRenderBufferDescriptor::default(),
        };
        let rb = HdRenderBufferSchema::get_from_parent(&ds);
        if !rb.is_defined() {
            return HdRenderBufferDescriptor::default();
        }

        let dimensions = rb
            .get_dimensions()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or_default();
        let multi_sampled = rb
            .get_multi_sampled()
            .map(|d| d.get_typed_value(0.0))
            .unwrap_or(false);

        HdRenderBufferDescriptor {
            dimensions,
            multi_sampled,
            ..HdRenderBufferDescriptor::default()
        }
    }

    // ----------------------------------------------------------------------- //
    // Light API
    // ----------------------------------------------------------------------- //

    /// Get light param value (G24: GetLightParamValue).
    ///
    /// Reads from the light data source container by param name.
    pub fn get_light_param_value(&self, id: &SdfPath, param_name: &TfToken) -> Value {
        let ds = match self.get_prim_ds(id) {
            Some(ds) => ds,
            None => return Value::empty(),
        };
        let light = HdLightSchema::get_from_parent(&ds);
        if !light.is_defined() {
            return Value::empty();
        }
        // Light schema exposes its container; read the named param
        if let Some(container) = light.get_container() {
            if let Some(child) = container.get(param_name) {
                if let Some(sampled) = child.as_sampled() {
                    return sampled.get_value(0.0);
                }
            }
        }
        Value::empty()
    }

    // ----------------------------------------------------------------------- //
    // Camera API
    // ----------------------------------------------------------------------- //

    /// Get camera param value (G24: GetCameraParamValue).
    ///
    /// Reads from the camera schema container by param name.
    pub fn get_camera_param_value(&self, camera_id: &SdfPath, param_name: &TfToken) -> Value {
        let ds = match self.get_prim_ds(camera_id) {
            Some(ds) => ds,
            None => return Value::empty(),
        };
        let camera = HdCameraSchema::get_from_parent(&ds);
        if !camera.is_defined() {
            return Value::empty();
        }
        if let Some(container) = camera.get_container() {
            if let Some(child) = container.get(param_name) {
                if let Some(sampled) = child.as_sampled() {
                    return sampled.get_value(0.0);
                }
            }
        }
        Value::empty()
    }

    // ----------------------------------------------------------------------- //
    // ExtComputation API
    // ----------------------------------------------------------------------- //

    /// Get ext computation primvar descriptors (G24: GetExtComputationPrimvarDescriptors).
    pub fn get_ext_computation_primvar_descriptors(
        &self,
        id: &SdfPath,
        interpolation: HdInterpolation,
    ) -> HdExtComputationPrimvarDescriptorVector {
        if let Some(cached) = self
            .prim_cache
            .read()
            .get(id)
            .and_then(|entry| entry.ext_comp_primvar_descriptors.as_ref().cloned())
        {
            return cached[primvar_descriptor_cache_index(interpolation)].clone();
        }

        let prim = self.get_input_prim(id);
        let Some(ds) = prim.data_source else {
            return Vec::new();
        };
        let ecp = HdExtComputationPrimvarsSchema::get_from_parent(&ds);
        if !ecp.is_defined() {
            return Vec::new();
        }
        let primvars = HdPrimvarsSchema::get_from_parent(&ds);
        let mut all = Box::new(std::array::from_fn(|_| Vec::new()));

        for name in ecp.get_ext_computation_primvar_names() {
            let pv = ecp.get_ext_computation_primvar(&name);
            if !pv.is_defined() {
                continue;
            }

            let interp = primvars
                .get_primvar_schema(&name)
                .get_interpolation()
                .map(|d| interpolation_from_token(&d.get_typed_value(0.0)))
                .unwrap_or(HdInterpolation::Constant);

            let Some(desc) =
                hd_ext_computation_primvar_descriptor_from_schema(&pv, name.clone(), interp)
            else {
                continue;
            };

            all[primvar_descriptor_cache_index(interp)].push(desc);
        }

        let result = all[primvar_descriptor_cache_index(interpolation)].clone();
        if let Some(cache_entry) = self.prim_cache.write().get_mut(id) {
            cache_entry.ext_comp_primvar_descriptors = Some(all);
        }
        result
    }

    /// Get ext computation scene input names (G24).
    pub fn get_ext_computation_scene_input_names(&self, computation_id: &SdfPath) -> Vec<TfToken> {
        let ds = match self.get_prim_ds(computation_id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let ec = HdExtComputationSchema::get_from_parent(&ds);
        if !ec.is_defined() {
            return Vec::new();
        }
        // Scene inputs = input values container names
        if let Some(iv) = ec.get_input_values() {
            return iv.get_names();
        }
        Vec::new()
    }

    /// Get ext computation input value (G24).
    pub fn get_ext_computation_input(&self, computation_id: &SdfPath, input: &TfToken) -> Value {
        let ds = match self.get_prim_ds(computation_id) {
            Some(ds) => ds,
            None => return Value::empty(),
        };
        let ec = HdExtComputationSchema::get_from_parent(&ds);
        if let Some(iv) = ec.get_input_values() {
            if let Some(child) = iv.get(input) {
                if let Some(sampled) = child.as_sampled() {
                    return sampled.get_value(0.0);
                }
            }
        }
        Value::empty()
    }

    /// Sample ext computation input (overload 1).
    pub fn sample_ext_computation_input(
        &self,
        computation_id: &SdfPath,
        input: &TfToken,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Value>,
    ) -> usize {
        self.sample_ext_computation_input_interval(
            computation_id,
            input,
            0.0,
            0.0,
            max_sample_count,
            out_times,
            out_values,
        )
    }

    /// Sample ext computation input over interval (overload 2).
    pub fn sample_ext_computation_input_interval(
        &self,
        computation_id: &SdfPath,
        input: &TfToken,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
        out_times: &mut Vec<f32>,
        out_values: &mut Vec<Value>,
    ) -> usize {
        out_times.clear();
        out_values.clear();
        let ds = match self.get_prim_ds(computation_id) {
            Some(ds) => ds,
            None => {
                out_times.push(0.0);
                out_values.push(Value::empty());
                return 1;
            }
        };
        let ec = HdExtComputationSchema::get_from_parent(&ds);
        if let Some(iv) = ec.get_input_values() {
            if let Some(child) = iv.get(input) {
                if let Some(sampled) = child.as_sampled() {
                    let (times, values) =
                        Self::sample_ds(sampled, start_time, end_time, max_sample_count);
                    *out_times = times;
                    *out_values = values;
                    return out_times.len();
                }
            }
        }
        out_times.push(0.0);
        out_values.push(Value::empty());
        1
    }

    /// Get ext computation input descriptors (G24).
    pub fn get_ext_computation_input_descriptors(
        &self,
        computation_id: &SdfPath,
    ) -> HdExtComputationInputDescriptorVector {
        let ds = match self.get_prim_ds(computation_id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let ec = HdExtComputationSchema::get_from_parent(&ds);
        if !ec.is_defined() {
            return Vec::new();
        }

        let mut result = Vec::new();
        if let Some(ic) = ec.get_input_computations() {
            for name in ic.get_names() {
                use crate::schema::HdExtComputationInputComputationSchema;
                let input_schema =
                    HdExtComputationInputComputationSchema::get_from_parent(&ic, &name);
                if !input_schema.is_defined() {
                    continue;
                }
                let source_computation = input_schema
                    .get_source_computation()
                    .map(|d| d.get_typed_value(0.0))
                    .unwrap_or_default();
                let source_output_name = input_schema
                    .get_source_computation_output_name()
                    .map(|d| d.get_typed_value(0.0))
                    .unwrap_or_default();
                result.push(HdExtComputationInputDescriptor {
                    name,
                    source_computation_id: source_computation,
                    source_computation_output_name: source_output_name,
                });
            }
        }
        result
    }

    /// Get ext computation output descriptors (G24).
    pub fn get_ext_computation_output_descriptors(
        &self,
        computation_id: &SdfPath,
    ) -> HdExtComputationOutputDescriptorVector {
        let ds = match self.get_prim_ds(computation_id) {
            Some(ds) => ds,
            None => return Vec::new(),
        };
        let ec = HdExtComputationSchema::get_from_parent(&ds);
        if !ec.is_defined() {
            return Vec::new();
        }

        let mut result = Vec::new();
        if let Some(oc) = ec.get_outputs() {
            for name in oc.get_names() {
                use crate::schema::HdExtComputationOutputSchema;
                let output_schema = HdExtComputationOutputSchema::get_from_parent(&oc, &name);
                if !output_schema.is_defined() {
                    continue;
                }
                let value_type = output_schema
                    .get_value_type()
                    .map(|d| d.get_typed_value(0.0))
                    .unwrap_or(Default::default());
                result.push(HdExtComputationOutputDescriptor { name, value_type });
            }
        }
        result
    }

    /// Get ext computation kernel (G24: GetExtComputationKernel).
    pub fn get_ext_computation_kernel(&self, computation_id: &SdfPath) -> String {
        let ds = match self.get_prim_ds(computation_id) {
            Some(ds) => ds,
            None => return String::new(),
        };
        let ec = HdExtComputationSchema::get_from_parent(&ds);
        if let Some(kernel_ds) = ec.get_glsl_kernel() {
            return kernel_ds.get_typed_value(0.0);
        }
        String::new()
    }

    /// Invoke ext computation (matches C++ signature with context param).
    ///
    /// Reads cpuCallback data source, extracts the callback, and calls Compute.
    pub fn invoke_ext_computation(
        &self,
        computation_id: &SdfPath,
        context: &mut dyn crate::HdExtComputationContext,
    ) {
        let prim = self.get_input_prim(computation_id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return,
        };
        let ext_comp = HdExtComputationSchema::get_from_parent(ds);
        let cb_ds = match ext_comp.get_cpu_callback() {
            Some(ds) => ds,
            None => return,
        };
        // Extract typed callback value from the sampled data source
        let sampled = match cb_ds.as_sampled() {
            Some(s) => s,
            None => return,
        };
        let val = sampled.get_value(0.0);
        if let Some(cb_val) = val.get::<HdExtComputationCpuCallbackValue>() {
            cb_val.get().compute(context);
        }
    }

    // ----------------------------------------------------------------------- //
    // Task API
    // ----------------------------------------------------------------------- //

    /// Get task render tags from legacy task schema.
    pub fn get_task_render_tags(&self, task_id: &SdfPath) -> Vec<TfToken> {
        let prim = self.get_input_prim(task_id);
        let ds = match prim.data_source {
            Some(ref ds) => ds,
            None => return Vec::new(),
        };
        let task_schema = HdLegacyTaskSchema::get_from_parent(ds);
        if let Some(render_tags_ds) = task_schema.get_render_tags() {
            return render_tags_ds.get_typed_value(0.0);
        }
        Vec::new()
    }

    // ----------------------------------------------------------------------- //
    // Sync API
    // ----------------------------------------------------------------------- //

    /// Sync prims (G24: Sync).
    ///
    /// Called by render index to synchronize dirty prims.
    ///
    /// Matches OpenUSD behavior by clearing the per-thread input-prim cache
    /// before rprim sync consumes scene-index data for the current frame.
    pub fn sync(&self, request: &mut HdSyncRequestVector) {
        if request.ids.is_empty() {
            return;
        }
        self.clear_input_prim_cache();
        for scene_delegate in self.collect_scene_delegates() {
            #[allow(unsafe_code)]
            let scene_delegate = unsafe { &mut *Arc::as_ptr(&scene_delegate).cast_mut() };
            scene_delegate.sync(request);
        }
    }

    /// Post-sync cleanup (G24: PostSyncCleanup).
    pub fn post_sync_cleanup(&self) {
        if let Some(scene_delegates) = self.scene_delegates.read().as_ref() {
            for scene_delegate in scene_delegates.clone() {
                #[allow(unsafe_code)]
                let scene_delegate = unsafe { &mut *Arc::as_ptr(&scene_delegate).cast_mut() };
                scene_delegate.post_sync_cleanup();
            }
        }
        self.clear_input_prim_cache();
    }
}

/// Convert interpolation token to HdInterpolation enum.
fn interpolation_from_token(token: &TfToken) -> HdInterpolation {
    use crate::schema::{
        PRIMVAR_CONSTANT, PRIMVAR_FACE_VARYING, PRIMVAR_INSTANCE, PRIMVAR_UNIFORM, PRIMVAR_VARYING,
        PRIMVAR_VERTEX,
    };
    if *token == *PRIMVAR_CONSTANT {
        HdInterpolation::Constant
    } else if *token == *PRIMVAR_UNIFORM {
        HdInterpolation::Uniform
    } else if *token == *PRIMVAR_VARYING {
        HdInterpolation::Varying
    } else if *token == *PRIMVAR_VERTEX {
        HdInterpolation::Vertex
    } else if *token == *PRIMVAR_FACE_VARYING {
        HdInterpolation::FaceVarying
    } else if *token == *PRIMVAR_INSTANCE {
        HdInterpolation::Instance
    } else {
        HdInterpolation::Constant
    }
}

// --------------------------------------------------------------------------- //
// G32: Free functions
// --------------------------------------------------------------------------- //

/// Build a HdPrimvarDescriptor from a primvar schema (G32).
///
/// Corresponds to C++ `HdPrimvarDescriptorFromSchema`.
pub fn hd_primvar_descriptor_from_schema(
    primvar: &crate::schema::HdPrimvarSchema,
    name: TfToken,
) -> Option<HdPrimvarDescriptor> {
    let interp_token = primvar.get_interpolation()?.get_typed_value(0.0);
    let interp = interpolation_from_token(&interp_token);
    let role = primvar
        .get_role()
        .map(|ds| ds.get_typed_value(0.0))
        .unwrap_or_default();
    let indexed = primvar.is_indexed();
    Some(HdPrimvarDescriptor::new(name, interp, role, indexed))
}

/// Build a HdExtComputationPrimvarDescriptor from schema (G32).
///
/// Corresponds to C++ `HdExtComputationPrimvarDescriptorFromSchema`.
pub fn hd_ext_computation_primvar_descriptor_from_schema(
    primvar: &crate::schema::HdExtComputationPrimvarSchema,
    name: TfToken,
    interpolation: HdInterpolation,
) -> Option<HdExtComputationPrimvarDescriptor> {
    if !primvar.is_defined() {
        return None;
    }
    let source_computation = primvar.get_source_computation()?.get_typed_value(0.0);
    let source_output_name = primvar
        .get_source_computation_output_name()
        .map(|d| d.get_typed_value(0.0))
        .unwrap_or_default();

    Some(HdExtComputationPrimvarDescriptor::new(
        name,
        interpolation,
        TfToken::default(),
        source_computation,
        source_output_name,
        Default::default(),
    ))
}

// --------------------------------------------------------------------------- //
// Observer impl - G29, G30, G31: RenderIndex integration
// --------------------------------------------------------------------------- //

impl HdSceneIndexObserver for HdSceneIndexAdapterSceneDelegate {
    /// Handle prims added from the scene index.
    ///
    /// G29: Classifies prim type and calls renderIndex->Insert* for
    /// rprim/sprim/bprim accordingly.
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.clear_input_prim_cache();
        if !entries.is_empty() {
            self.invalidate_scene_delegate_cache();
        }

        let mut cache = self.prim_cache.write();

        for entry in entries {
            let mut is_resync = false;
            let existing_type = cache.get(&entry.prim_path).map(|old| old.prim_type.clone());

            // Match OpenUSD `_PrimAdded`:
            // 1. missing prim -> insert
            // 2. existing prim with different type (or task) -> remove old, insert new
            // 3. existing prim with same non-task type -> resync without reinserting
            if let Some(existing_type) = existing_type {
                if entry.prim_type != existing_type || entry.prim_type == "task" {
                    if let Some(mut ri_ptr) = self.render_index {
                        let ri = unsafe { ri_ptr.as_mut() };
                        let old_kind = classify_prim_type(&existing_type, Some(ri));
                        match old_kind {
                            PrimKind::Rprim => {
                                ri.remove_rprim(&entry.prim_path);
                            }
                            PrimKind::Sprim => {
                                ri.remove_sprim(&existing_type, &entry.prim_path);
                            }
                            PrimKind::Bprim => {
                                ri.remove_bprim(&existing_type, &entry.prim_path);
                            }
                            PrimKind::Instancer => {
                                ri.remove_instancer(&entry.prim_path);
                            }
                            PrimKind::GeomSubset => {
                                mark_rprim_dirty_from_notice(
                                    ri,
                                    &entry.prim_path.get_parent_path(),
                                    HdRprimDirtyBits::DIRTY_TOPOLOGY,
                                );
                            }
                            PrimKind::Task => {
                                ri.remove_task(&entry.prim_path);
                            }
                            PrimKind::Unknown => {}
                        }
                    }
                } else {
                    is_resync = true;
                }
            }

            if !is_resync {
                if let Some(mut ri_ptr) = self.render_index {
                    let ri = unsafe { ri_ptr.as_mut() };
                    let kind = classify_prim_type(&entry.prim_type, Some(ri));
                    match kind {
                        PrimKind::Rprim => {
                            ri.insert_rprim(&entry.prim_type, &self.delegate_id, &entry.prim_path);
                        }
                        PrimKind::Sprim => {
                            ri.insert_sprim(&entry.prim_type, &self.delegate_id, &entry.prim_path);
                        }
                        PrimKind::Bprim => {
                            ri.insert_bprim(&entry.prim_type, &self.delegate_id, &entry.prim_path);
                        }
                        PrimKind::Instancer => {
                            ri.insert_instancer(&self.delegate_id, &entry.prim_path);
                        }
                        PrimKind::GeomSubset => {
                            mark_rprim_dirty_from_notice(
                                ri,
                                &entry.prim_path.get_parent_path(),
                                HdRprimDirtyBits::DIRTY_TOPOLOGY,
                            );
                        }
                        PrimKind::Task | PrimKind::Unknown => {}
                    }
                }
            }

            if let Some(cache_entry) = cache.get_mut(&entry.prim_path) {
                cache_entry.prim_type = entry.prim_type.clone();
                cache_entry.primvar_descriptors = None;
                cache_entry.cached_locator_set = None;
                cache_entry.cached_dirty_bits = 0;
                cache_entry.cached_prim_type = TfToken::default();
            } else {
                cache.insert(
                    entry.prim_path.clone(),
                    PrimCacheEntry {
                        prim_type: entry.prim_type.clone(),
                        primvar_descriptors: None,
                        ext_comp_primvar_descriptors: None,
                        cached_locator_set: None,
                        cached_dirty_bits: 0,
                        cached_prim_type: TfToken::default(),
                    },
                );
            }

            if is_resync {
                let all_dirty = HdDataSourceLocatorSet::universal();
                if let Some(mut ri_ptr) = self.render_index {
                    let ri = unsafe { ri_ptr.as_mut() };
                    let kind = classify_prim_type(&entry.prim_type, Some(ri));
                    match kind {
                        PrimKind::Rprim => {
                            let dirty_bits =
                                HdDirtyBitsTranslator::rprim_locator_set_to_dirty_bits(
                                    &entry.prim_type,
                                    &all_dirty,
                                );
                            if dirty_bits != 0 {
                                mark_rprim_dirty_from_notice(ri, &entry.prim_path, dirty_bits);
                            }
                        }
                        PrimKind::Sprim => {
                            let dirty_bits = HdDirtyBitsTranslator::sprim_locator_set_to_dirty_bits(
                                &entry.prim_type,
                                &all_dirty,
                            );
                            if dirty_bits != 0 {
                                ri.get_change_tracker_mut()
                                    .mark_sprim_dirty(&entry.prim_path, dirty_bits);
                            }
                        }
                        PrimKind::Bprim => {
                            let dirty_bits = HdDirtyBitsTranslator::bprim_locator_set_to_dirty_bits(
                                &entry.prim_type,
                                &all_dirty,
                            );
                            if dirty_bits != 0 {
                                ri.get_change_tracker_mut()
                                    .mark_bprim_dirty(&entry.prim_path, dirty_bits);
                            }
                        }
                        PrimKind::Instancer => {
                            let dirty_bits =
                                HdDirtyBitsTranslator::instancer_locator_set_to_dirty_bits(
                                    &entry.prim_type,
                                    &all_dirty,
                                );
                            if dirty_bits != 0 {
                                ri.get_change_tracker_mut()
                                    .mark_instancer_dirty(&entry.prim_path, dirty_bits);
                            }
                        }
                        PrimKind::GeomSubset => {
                            mark_rprim_dirty_from_notice(
                                ri,
                                &entry.prim_path.get_parent_path(),
                                HdRprimDirtyBits::DIRTY_TOPOLOGY,
                            );
                        }
                        PrimKind::Task | PrimKind::Unknown => {}
                    }
                }
            }

            if entry.prim_type == "geomSubset" {
                self.add_geom_subset_parent(&entry.prim_path.get_parent_path());
            }
        }
    }

    /// Handle prims removed from the scene index.
    ///
    /// G31: Removes from render index (rprim/sprim/bprim) based on cached type.
    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.clear_input_prim_cache();
        if !entries.is_empty() {
            self.invalidate_scene_delegate_cache();
        }

        let mut cache = self.prim_cache.write();

        for entry in entries {
            if entry.prim_path.is_absolute_root_path() {
                if let Some(mut ri_ptr) = self.render_index {
                    let ri = unsafe { ri_ptr.as_mut() };
                    ri.clear();
                }
                cache.clear();
                self.geom_subset_parents.write().clear();
                continue;
            }

            let Some(cache_entry) = cache.get(&entry.prim_path) else {
                continue;
            };

            let has_descendants = cache
                .keys()
                .any(|path| path != &entry.prim_path && path.has_prefix(&entry.prim_path));

            if let Some(mut ri_ptr) = self.render_index {
                let ri = unsafe { ri_ptr.as_mut() };
                if has_descendants {
                    ri.remove_subtree(&entry.prim_path, &self.delegate_id);
                } else {
                    let kind = classify_prim_type(&cache_entry.prim_type, Some(ri));
                    match kind {
                        PrimKind::Rprim => {
                            ri.remove_rprim(&entry.prim_path);
                        }
                        PrimKind::Sprim => {
                            ri.remove_sprim(&cache_entry.prim_type, &entry.prim_path);
                        }
                        PrimKind::Bprim => {
                            ri.remove_bprim(&cache_entry.prim_type, &entry.prim_path);
                        }
                        PrimKind::Instancer => {
                            ri.remove_instancer(&entry.prim_path);
                        }
                        PrimKind::GeomSubset => {
                            mark_rprim_dirty_from_notice(
                                ri,
                                &entry.prim_path.get_parent_path(),
                                HdRprimDirtyBits::DIRTY_TOPOLOGY,
                            );
                        }
                        PrimKind::Task => {
                            ri.remove_task(&entry.prim_path);
                        }
                        PrimKind::Unknown => {}
                    }
                }
            }

            if has_descendants {
                cache.retain(|path, _| !path.has_prefix(&entry.prim_path));
                self.geom_subset_parents
                    .write()
                    .retain(|path| !path.has_prefix(&entry.prim_path));
            } else {
                cache.remove(&entry.prim_path);
                self.geom_subset_parents
                    .write()
                    .retain(|path| path != &entry.prim_path);
            }
        }
    }

    /// Handle prims dirtied from the scene index.
    ///
    /// G30: Translates locator sets to dirty bits via HdDirtyBitsTranslator,
    /// then marks dirty in the render index's change tracker.
    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.clear_input_prim_cache();

        if flo_debug_enabled() {
            let summary = summarize_dirtied_entries(entries);
            eprintln!(
                "[dirty-trace] stage=scene_index_adapter emitter={} total={} unique={} dup_paths={} dup_instances={} first={}",
                self.get_delegate_id(),
                summary.total,
                summary.unique_paths,
                summary.duplicate_paths,
                summary.duplicate_instances,
                summary.first_path,
            );
        }

        let mut cache = self.prim_cache.write();

        for entry in entries {
            if std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some() {
                eprintln!(
                    "[scene_delegate] dirtied path={} locators={:?}",
                    entry.prim_path,
                    entry.dirty_locators
                );
            }
            if let Some(cache_entry) = cache.get_mut(&entry.prim_path) {
                if let Some(mut ri_ptr) = self.render_index {
                    let ri = unsafe { ri_ptr.as_mut() };
                    let kind = classify_prim_type(&cache_entry.prim_type, Some(ri));
                    let dirty_bits = if entry.dirty_locators.is_empty() {
                        0
                    } else {
                        match kind {
                            PrimKind::Rprim => {
                                if cache_entry.cached_locator_set.as_ref()
                                    == Some(&entry.dirty_locators)
                                    && cache_entry.cached_prim_type == cache_entry.prim_type
                                {
                                    cache_entry.cached_dirty_bits
                                } else {
                                    let bits =
                                        HdDirtyBitsTranslator::rprim_locator_set_to_dirty_bits(
                                            &cache_entry.prim_type,
                                            &entry.dirty_locators,
                                        );
                                    cache_entry.cached_locator_set =
                                        Some(entry.dirty_locators.clone());
                                    cache_entry.cached_prim_type = cache_entry.prim_type.clone();
                                    cache_entry.cached_dirty_bits = bits;
                                    bits
                                }
                            }
                            PrimKind::Sprim => {
                                HdDirtyBitsTranslator::sprim_locator_set_to_dirty_bits(
                                    &cache_entry.prim_type,
                                    &entry.dirty_locators,
                                )
                            }
                            PrimKind::Bprim => {
                                HdDirtyBitsTranslator::bprim_locator_set_to_dirty_bits(
                                    &cache_entry.prim_type,
                                    &entry.dirty_locators,
                                )
                            }
                            PrimKind::Instancer => {
                                HdDirtyBitsTranslator::instancer_locator_set_to_dirty_bits(
                                    &cache_entry.prim_type,
                                    &entry.dirty_locators,
                                )
                            }
                            PrimKind::Task => {
                                HdDirtyBitsTranslator::task_locator_set_to_dirty_bits(
                                    &entry.dirty_locators,
                                )
                            }
                            PrimKind::GeomSubset => 0,
                            PrimKind::Unknown => HD_DIRTY_ALL,
                        }
                    };
                    if std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some() {
                        eprintln!(
                            "[scene_delegate] kind={:?} prim={} bits=0x{:x}",
                            kind,
                            entry.prim_path,
                            dirty_bits
                        );
                    }

                    if dirty_bits != 0 {
                        match kind {
                            PrimKind::Rprim => {
                                mark_rprim_dirty_from_notice(ri, &entry.prim_path, dirty_bits);
                            }
                            PrimKind::Sprim => {
                                ri.get_change_tracker_mut()
                                    .mark_sprim_dirty(&entry.prim_path, dirty_bits);
                            }
                            PrimKind::Bprim => {
                                ri.get_change_tracker_mut()
                                    .mark_bprim_dirty(&entry.prim_path, dirty_bits);
                            }
                            PrimKind::Instancer => {
                                ri.get_change_tracker_mut()
                                    .mark_instancer_dirty(&entry.prim_path, dirty_bits);
                            }
                            PrimKind::Task => {
                                ri.get_change_tracker_mut()
                                    .mark_task_dirty(&entry.prim_path, dirty_bits);
                            }
                            PrimKind::GeomSubset => {
                                mark_rprim_dirty_from_notice(
                                    ri,
                                    &entry.prim_path.get_parent_path(),
                                    HdRprimDirtyBits::DIRTY_TOPOLOGY,
                                );
                            }
                            PrimKind::Unknown => {}
                        }
                    } else if kind == PrimKind::GeomSubset {
                        mark_rprim_dirty_from_notice(
                            ri,
                            &entry.prim_path.get_parent_path(),
                            HdRprimDirtyBits::DIRTY_TOPOLOGY,
                        );
                    }
                }

                if invalidates_primvar_descriptors(&entry.dirty_locators) {
                    cache_entry.primvar_descriptors = None;
                }
                if entry
                    .dirty_locators
                    .intersects_locator(&HdExtComputationPrimvarsSchema::get_default_locator())
                {
                    cache_entry.ext_comp_primvar_descriptors = None;
                }
            }
        }
    }

    /// Handle prims renamed from the scene index.
    ///
    /// Match OpenUSD directly: decompose renames into removed + added notices.
    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let (removed, added) =
            crate::scene_index::observer::convert_prims_renamed_to_removed_and_added(sender, entries);

        if !removed.is_empty() {
            self.prims_removed(sender, &removed);
        }
        if !added.is_empty() {
            self.prims_added(sender, &added);
        }
    }
}

// --------------------------------------------------------------------------- //
// HdSceneDelegate trait impl
// --------------------------------------------------------------------------- //

/// Bridges HdSceneIndexAdapterSceneDelegate into the HdSceneDelegate trait.
///
/// All query methods delegate to the inherent impl above, adapting from the
/// C++ out-param style (used internally) to the Rust trait return style.
/// Corresponds to C++ class inheritance: HdSceneIndexAdapterSceneDelegate : HdSceneDelegate.
impl crate::prim::HdSceneDelegate for HdSceneIndexAdapterSceneDelegate {
    fn get_dirty_bits(&self, _id: &SdfPath) -> HdDirtyBits {
        // Scene index adapter tracks dirty state via observer notifications, not per-prim bits.
        0
    }

    fn mark_clean(&mut self, _id: &SdfPath, _bits: HdDirtyBits) {
        // No-op: scene index adapter clears state through observer events.
    }

    fn get_delegate_id(&self) -> SdfPath {
        self.delegate_id.clone()
    }

    fn get_instancer_id(&self, prim_id: &SdfPath) -> SdfPath {
        // Disambiguate: call inherent method, not the trait method
        HdSceneIndexAdapterSceneDelegate::get_instancer_id(self, prim_id)
    }

    fn get_transform(&self, id: &SdfPath) -> Matrix4d {
        HdSceneIndexAdapterSceneDelegate::get_transform(self, id)
    }

    fn get_extent(&self, id: &SdfPath) -> usd_gf::Range3d {
        HdSceneIndexAdapterSceneDelegate::get_extent(self, id)
    }

    fn get_visible(&self, id: &SdfPath) -> bool {
        HdSceneIndexAdapterSceneDelegate::get_visible(self, id)
    }

    fn get_double_sided(&self, id: &SdfPath) -> bool {
        HdSceneIndexAdapterSceneDelegate::get_double_sided(self, id)
    }

    fn get_mesh_topology(&self, id: &SdfPath) -> crate::prim::mesh::HdMeshTopology {
        HdSceneIndexAdapterSceneDelegate::get_mesh_topology(self, id)
    }

    fn get_basis_curves_topology(
        &self,
        id: &SdfPath,
    ) -> crate::prim::basis_curves::HdBasisCurvesTopology {
        // Convert from canonical basis_curves_topology type to the prim-local type used by the trait.
        let topo = HdSceneIndexAdapterSceneDelegate::get_basis_curves_topology(self, id);
        convert_basis_curves_topology(topo)
    }

    fn get_subdiv_tags(&self, id: &SdfPath) -> SubdivTags {
        HdSceneIndexAdapterSceneDelegate::get_subdiv_tags(self, id)
    }

    fn get_cull_style(&self, id: &SdfPath) -> HdCullStyle {
        HdSceneIndexAdapterSceneDelegate::get_cull_style(self, id)
    }

    fn get_shading_style(&self, id: &SdfPath) -> usd_vt::Value {
        HdSceneIndexAdapterSceneDelegate::get_shading_style(self, id)
    }

    fn get_display_style(&self, id: &SdfPath) -> HdDisplayStyle {
        HdSceneIndexAdapterSceneDelegate::get_display_style(self, id)
    }

    fn get(&self, id: &SdfPath, key: &usd_tf::Token) -> usd_vt::Value {
        HdSceneIndexAdapterSceneDelegate::get(self, id, key)
    }

    fn get_indexed_primvar(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
    ) -> (usd_vt::Value, Option<Vec<i32>>) {
        // Adapt out-param style to trait (Value, Option<indices>) return style
        let mut indices = Vec::new();
        let val =
            HdSceneIndexAdapterSceneDelegate::get_indexed_primvar(self, id, key, &mut indices);
        let opt = if indices.is_empty() {
            None
        } else {
            Some(indices)
        };
        (val, opt)
    }

    fn get_repr_selector(&self, id: &SdfPath) -> crate::prim::HdReprSelector {
        HdSceneIndexAdapterSceneDelegate::get_repr_selector(self, id)
    }

    fn get_render_tag(&self, id: &SdfPath) -> usd_tf::Token {
        HdSceneIndexAdapterSceneDelegate::get_render_tag(self, id)
    }

    fn get_categories(&self, id: &SdfPath) -> Vec<usd_tf::Token> {
        HdSceneIndexAdapterSceneDelegate::get_categories(self, id)
    }

    fn get_instance_categories(&self, instancer_id: &SdfPath) -> Vec<Vec<usd_tf::Token>> {
        HdSceneIndexAdapterSceneDelegate::get_instance_categories(self, instancer_id)
    }

    fn get_coord_sys_bindings(&self, id: &SdfPath) -> Option<HdIdVectorSharedPtr> {
        HdSceneIndexAdapterSceneDelegate::get_coord_sys_bindings(self, id)
    }

    fn get_material_id(&self, id: &SdfPath) -> Option<SdfPath> {
        HdSceneIndexAdapterSceneDelegate::get_material_id(self, id)
    }

    fn get_material_resource(&self, material_id: &SdfPath) -> usd_vt::Value {
        HdSceneIndexAdapterSceneDelegate::get_material_resource(self, material_id)
    }

    fn get_render_buffer_descriptor(&self, id: &SdfPath) -> HdRenderBufferDescriptor {
        HdSceneIndexAdapterSceneDelegate::get_render_buffer_descriptor(self, id)
    }

    fn get_light_param_value(&self, id: &SdfPath, param_name: &usd_tf::Token) -> usd_vt::Value {
        HdSceneIndexAdapterSceneDelegate::get_light_param_value(self, id, param_name)
    }

    fn get_camera_param_value(
        &self,
        camera_id: &SdfPath,
        param_name: &usd_tf::Token,
    ) -> usd_vt::Value {
        HdSceneIndexAdapterSceneDelegate::get_camera_param_value(self, camera_id, param_name)
    }

    fn get_volume_field_descriptors(&self, volume_id: &SdfPath) -> HdVolumeFieldDescriptorVector {
        HdSceneIndexAdapterSceneDelegate::get_volume_field_descriptors(self, volume_id)
    }

    fn get_ext_computation_scene_input_names(
        &self,
        computation_id: &SdfPath,
    ) -> Vec<usd_tf::Token> {
        HdSceneIndexAdapterSceneDelegate::get_ext_computation_scene_input_names(
            self,
            computation_id,
        )
    }

    fn get_ext_computation_input_descriptors(
        &self,
        computation_id: &SdfPath,
    ) -> HdExtComputationInputDescriptorVector {
        HdSceneIndexAdapterSceneDelegate::get_ext_computation_input_descriptors(
            self,
            computation_id,
        )
    }

    fn get_ext_computation_output_descriptors(
        &self,
        computation_id: &SdfPath,
    ) -> HdExtComputationOutputDescriptorVector {
        HdSceneIndexAdapterSceneDelegate::get_ext_computation_output_descriptors(
            self,
            computation_id,
        )
    }

    fn get_ext_computation_primvar_descriptors(
        &self,
        id: &SdfPath,
        interpolation: HdInterpolation,
    ) -> HdExtComputationPrimvarDescriptorVector {
        HdSceneIndexAdapterSceneDelegate::get_ext_computation_primvar_descriptors(
            self,
            id,
            interpolation,
        )
    }

    fn get_ext_computation_input(
        &self,
        computation_id: &SdfPath,
        input: &usd_tf::Token,
    ) -> usd_vt::Value {
        HdSceneIndexAdapterSceneDelegate::get_ext_computation_input(self, computation_id, input)
    }

    fn get_ext_computation_kernel(&self, computation_id: &SdfPath) -> String {
        HdSceneIndexAdapterSceneDelegate::get_ext_computation_kernel(self, computation_id)
    }

    fn invoke_ext_computation(
        &mut self,
        computation_id: &SdfPath,
        context: &mut dyn crate::HdExtComputationContext,
    ) {
        HdSceneIndexAdapterSceneDelegate::invoke_ext_computation(self, computation_id, context);
    }

    fn get_primvar_descriptors(
        &self,
        id: &SdfPath,
        interpolation: HdInterpolation,
    ) -> HdPrimvarDescriptorVector {
        HdSceneIndexAdapterSceneDelegate::get_primvar_descriptors(self, id, interpolation)
    }

    fn get_task_render_tags(&self, task_id: &SdfPath) -> Vec<usd_tf::Token> {
        HdSceneIndexAdapterSceneDelegate::get_task_render_tags(self, task_id)
    }

    fn sync(&mut self, request: &mut HdSyncRequestVector) {
        HdSceneIndexAdapterSceneDelegate::sync(self, request);
    }

    fn post_sync_cleanup(&mut self) {
        HdSceneIndexAdapterSceneDelegate::post_sync_cleanup(self);
    }

    // Sampling methods: adapt out-param style to trait Vec-return style.

    fn sample_transform(&self, id: &SdfPath, max_sample_count: usize) -> Vec<(f32, Matrix4d)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_transform(
            self,
            id,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }

    fn sample_transform_interval(
        &self,
        id: &SdfPath,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_transform_interval(
            self,
            id,
            start_time,
            end_time,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }

    fn sample_primvar(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_primvar(
            self,
            id,
            key,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }

    fn sample_primvar_interval(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_primvar_interval(
            self,
            id,
            key,
            start_time,
            end_time,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }

    fn sample_indexed_primvar(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value, Option<Vec<i32>>)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        let mut idx_vec: Vec<Vec<i32>> = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_indexed_primvar(
            self,
            id,
            key,
            max_sample_count,
            &mut times,
            &mut values,
            &mut idx_vec,
        );
        times
            .into_iter()
            .zip(values)
            .zip(idx_vec)
            .map(|((t, v), idx)| (t, v, if idx.is_empty() { None } else { Some(idx) }))
            .collect()
    }

    fn sample_indexed_primvar_interval(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value, Option<Vec<i32>>)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        let mut idx_vec: Vec<Vec<i32>> = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_indexed_primvar_interval(
            self,
            id,
            key,
            start_time,
            end_time,
            max_sample_count,
            &mut times,
            &mut values,
            &mut idx_vec,
        );
        times
            .into_iter()
            .zip(values)
            .zip(idx_vec)
            .map(|((t, v), idx)| (t, v, if idx.is_empty() { None } else { Some(idx) }))
            .collect()
    }

    fn get_instance_indices(&self, instancer_id: &SdfPath, prototype_id: &SdfPath) -> Vec<i32> {
        HdSceneIndexAdapterSceneDelegate::get_instance_indices(self, instancer_id, prototype_id)
    }

    fn get_instancer_transform(&self, instancer_id: &SdfPath) -> Matrix4d {
        HdSceneIndexAdapterSceneDelegate::get_instancer_transform(self, instancer_id)
    }

    fn get_instancer_prototypes(&self, instancer_id: &SdfPath) -> Vec<SdfPath> {
        HdSceneIndexAdapterSceneDelegate::get_instancer_prototypes(self, instancer_id)
    }

    fn sample_instancer_transform(
        &self,
        instancer_id: &SdfPath,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_instancer_transform(
            self,
            instancer_id,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }

    fn sample_instancer_transform_interval(
        &self,
        instancer_id: &SdfPath,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_instancer_transform_interval(
            self,
            instancer_id,
            start_time,
            end_time,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }

    fn sample_ext_computation_input(
        &self,
        computation_id: &SdfPath,
        input: &usd_tf::Token,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_ext_computation_input(
            self,
            computation_id,
            input,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }

    fn sample_ext_computation_input_interval(
        &self,
        computation_id: &SdfPath,
        input: &usd_tf::Token,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        let mut times = Vec::new();
        let mut values = Vec::new();
        HdSceneIndexAdapterSceneDelegate::sample_ext_computation_input_interval(
            self,
            computation_id,
            input,
            start_time,
            end_time,
            max_sample_count,
            &mut times,
            &mut values,
        );
        times.into_iter().zip(values).collect()
    }
}

/// Convert `HdBasisCurvesTopology` from the canonical module to the prim-local type.
///
/// The trait `HdSceneDelegate::get_basis_curves_topology` returns the prim-local
/// `prim::basis_curves::HdBasisCurvesTopology`. The adapter's inherent method returns
/// the richer `basis_curves_topology::HdBasisCurvesTopology`. This bridges the two.
fn convert_basis_curves_topology(
    src: crate::basis_curves_topology::HdBasisCurvesTopology,
) -> crate::prim::basis_curves::HdBasisCurvesTopology {
    use crate::prim::basis_curves::{HdCurveBasis, HdCurveType, HdCurveWrap};

    // Map curve_type token to enum
    let curve_type = match src.get_curve_type().as_str() {
        "cubic" => Some(HdCurveType::Cubic),
        _ => Some(HdCurveType::Linear),
    };

    // Map basis token to enum
    let basis = match src.get_curve_basis().as_str() {
        "bezier" => Some(HdCurveBasis::Bezier),
        "bspline" => Some(HdCurveBasis::BSpline),
        "catmullRom" => Some(HdCurveBasis::CatmullRom),
        "hermite" => Some(HdCurveBasis::Hermite),
        _ => None,
    };

    // Map wrap token to enum
    let wrap = match src.get_curve_wrap().as_str() {
        "periodic" => HdCurveWrap::Periodic,
        "pinned" => HdCurveWrap::Pinned,
        _ => HdCurveWrap::Nonperiodic,
    };

    crate::prim::basis_curves::HdBasisCurvesTopology {
        curve_vertex_counts: src.get_curve_vertex_counts().to_vec(),
        basis,
        curve_type,
        wrap,
    }
}

#[cfg(test)]
mod tests {
    use super::compute_primvar_descriptors;
    use crate::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};
    use crate::schema::{HdPrimvarSchema, HdPrimvarsSchema};
    use crate::scene_delegate::HdPrimvarDescriptor;
    use crate::HdInterpolation;
    use usd_sdf::Path as SdfPath;
    use usd_tf::Token;

    #[test]
    fn test_compute_primvar_descriptors_populates_all_interpolation_buckets() {
        let normals = HdPrimvarSchema::build_retained(
            None,
            None,
            None,
            Some(HdRetainedTypedSampledDataSource::new(Token::new("faceVarying"))),
            Some(HdRetainedTypedSampledDataSource::new(Token::new("normal"))),
            None,
            None,
        );
        let display_color = HdPrimvarSchema::build_retained(
            None,
            None,
            None,
            Some(HdRetainedTypedSampledDataSource::new(Token::new("vertex"))),
            Some(HdRetainedTypedSampledDataSource::new(Token::new("color"))),
            None,
            None,
        );
        let display_opacity = HdPrimvarSchema::build_retained(
            None,
            None,
            None,
            Some(HdRetainedTypedSampledDataSource::new(Token::new("constant"))),
            None,
            None,
            None,
        );
        let primvars = HdPrimvarsSchema::build_retained(
            &[
                Token::new("normals"),
                Token::new("displayColor"),
                Token::new("displayOpacity"),
            ],
            &[
                normals.clone() as _,
                display_color.clone() as _,
                display_opacity.clone() as _,
            ],
        );
        let prim: crate::data_source::HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[(Token::new("primvars"), primvars)]);

        let descriptors = compute_primvar_descriptors(&SdfPath::absolute_root(), &prim);

        assert_eq!(
            descriptors[HdInterpolation::FaceVarying as usize],
            vec![HdPrimvarDescriptor::new(
                Token::new("normals"),
                HdInterpolation::FaceVarying,
                Token::new("normal"),
                false,
            )]
        );
        assert_eq!(
            descriptors[HdInterpolation::Vertex as usize],
            vec![HdPrimvarDescriptor::new(
                Token::new("displayColor"),
                HdInterpolation::Vertex,
                Token::new("color"),
                false,
            )]
        );
        assert_eq!(
            descriptors[HdInterpolation::Constant as usize],
            vec![HdPrimvarDescriptor::new(
                Token::new("displayOpacity"),
                HdInterpolation::Constant,
                Token::default(),
                false,
            )]
        );
    }
}
