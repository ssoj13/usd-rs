//! HdRenderIndex - Central registry for scene primitives.
//!
//! The render index is a flattened representation of the scene graph,
//! holding handles to all scene objects (rprims, sprims, bprims).
//!
//! # Architecture
//!
//! - Tied to a single HdRenderDelegate for creating/destroying prims
//! - Can have multiple HdSceneDelegates providing data
//! - Tracks changes via HdChangeTracker
//! - Coordinates sync operations via multi-phase SyncAll
//! - Not tied to a specific viewport (can be shared)
//!
//! # SyncAll Pipeline (matches C++ phases)
//!
//! 1. Render delegate Update (scene index emulation)
//! 2. Bprim sync (dirty bprims via tracker)
//! 3. Sprim sync (dirty sprims via tracker)
//! 4. Task sync (dirty tasks via tracker)
//! 5. Rprim sync: collect dirty -> sync instancers -> sync rprims -> clean up
//! 6. Commit resources via render delegate
//!
//! # Prim Types
//!
//! - **Rprims**: Renderable geometry (mesh, curves, points, volume)
//! - **Sprims**: State objects (camera, light, material, coord sys)
//! - **Bprims**: Buffer objects (render buffer, ext computation)
//! - **Instancers**: Geometry instancing managers

use super::driver::HdDriverVector;
use super::render_delegate::{HdRenderDelegate, HdRprimCollection};
use super::task::HdTaskSharedPtr;
use super::task_context::HdTaskContext;
use crate::change_tracker::HdChangeTracker;
use crate::flattened_xform_data_source_provider::{
    read_debug_flattened_xform_stats, reset_debug_flattened_xform_stats,
};
use crate::flo_debug::flo_debug_enabled;
use crate::prim::instancer::HdInstancer;
use crate::scene_index::base::HdSceneIndexHandle;
use crate::scene_index::flattening::{
    read_debug_flattening_xform_get_stats, reset_debug_flattening_xform_get_stats,
};
use crate::scene_index::legacy_prim::HdLegacyPrimSceneIndex;
use crate::scene_index::merging::HdMergingSceneIndex;
use crate::scene_index::notice_batching::HdNoticeBatchingSceneIndex;
use crate::scene_index::observer::HdSceneIndexObserverHandle;
use crate::scene_index::prefixing::HdPrefixingSceneIndex;
use crate::scene_index_adapter_scene_delegate::{
    HdSceneIndexAdapterSceneDelegate, read_debug_transform_stats, reset_debug_transform_stats,
};
use crate::schema::xform::{read_debug_xform_schema_stats, reset_debug_xform_schema_stats};
use crate::types::HdDirtyBits;
use parking_lot::RwLock;
use std::any::Any;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Once};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Null scene delegate providing default values for all queries.
///
/// Used internally when syncing prims that don't have a live scene delegate
/// reference (the render index stores delegate IDs, not references).
/// In a complete pipeline, the actual scene delegate would be passed through
/// from the application layer.
struct NullSceneDelegate;

impl crate::prim::HdSceneDelegate for NullSceneDelegate {
    fn get_dirty_bits(&self, _id: &SdfPath) -> HdDirtyBits {
        0
    }
    fn mark_clean(&mut self, _id: &SdfPath, _bits: HdDirtyBits) {}
    fn get_instancer_id(&self, _prim_id: &SdfPath) -> SdfPath {
        SdfPath::default()
    }
}

/// Static null scene delegate for sync dispatch.
static NULL_SCENE_DELEGATE: NullSceneDelegate = NullSceneDelegate;

/// Shared pointer to render delegate.
pub type HdRenderDelegateSharedPtr = Arc<RwLock<dyn HdRenderDelegate>>;

/// Opaque prim handle (type-erased).
///
/// Since Rprim/Sprim/Bprim traits are not object-safe (they have associated constants),
/// we store them as opaque Box<dyn Any> and let the render delegate manage them.
/// Render delegate's sync_bprim/sync_sprim/sync_rprim downcast via as_any_mut().
pub type HdPrimHandle = Box<dyn Any + Send + Sync>;

// ---------------------------------------------------------------------------
// Object-safe sync dispatch trampoline traits.
//
// HdBprim/HdSprim/HdRprim are NOT object-safe (associated constants).
// These thin object-safe wrappers forward sync() calls so render_index can
// invoke them directly without going through the render delegate.
// render_delegate.sync_* still works for backends that want full control.
// ---------------------------------------------------------------------------

/// Object-safe bprim sync interface.
pub trait HdBprimSync: Send + Sync {
    /// Delegate-driven sync. Called from SyncAll bprim phase.
    fn sync_dyn(
        &mut self,
        delegate: &dyn crate::prim::HdSceneDelegate,
        render_param: Option<&dyn crate::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    );
    /// For render_delegate downcast to concrete type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Object-safe sprim sync interface.
pub trait HdSprimSync: Send + Sync {
    /// Delegate-driven sync. Called from SyncAll sprim phase.
    fn sync_dyn(
        &mut self,
        delegate: &dyn crate::prim::HdSceneDelegate,
        render_param: Option<&dyn crate::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    );
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Object-safe rprim sync interface.
pub trait HdRprimSync: Send + Sync {
    /// Delegate-driven sync. Called from SyncAll rprim phase.
    fn sync_dyn(
        &mut self,
        delegate: &dyn crate::prim::HdSceneDelegate,
        render_param: Option<&dyn crate::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
        repr_token: &Token,
    );
    /// Optional: propagate dirty bits before sync (matches C++ _PropagateDirtyBits).
    fn propagate_dirty_bits_dyn(&self, bits: HdDirtyBits) -> HdDirtyBits {
        bits
    }
    /// Optional: initialize repr on first use (matches C++ _InitRepr).
    fn init_repr_dyn(&mut self, _repr_token: &Token, _dirty_bits: &mut HdDirtyBits) {}
    /// Optional: skip propagation and sync for invisible prims without NewRepr.
    /// Matches C++ HdRprim::CanSkipDirtyBitPropagationAndSync (rprim.cpp:57-67).
    fn can_skip_dirty_bit_propagation_and_sync_dyn(&self, _bits: HdDirtyBits) -> bool {
        false
    }
    /// Optional: update the authored repr selector (matches C++ UpdateReprSelector).
    /// Called in PreSync when InitRepr|DirtyRepr bits are set.
    fn set_repr_selector_dyn(&mut self, _repr_sel: crate::prim::HdReprSelector) {}
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// For read-only downcast (e.g. to read draw items from the typed prim).
    fn as_any_ref(&self) -> &dyn Any;
}

/// Blanket adapter: wraps any T: HdBprim into a HdBprimSync object.
///
/// render_delegate creates `Box<BprimAdapter<T>>` via `BprimAdapter::new(prim)`.
/// render_index calls `.sync_dyn()`. render_delegate downcasts via `.as_any_mut()`.
pub struct BprimAdapter<T: crate::prim::bprim::HdBprim + Send + Sync + 'static>(pub T);

impl<T: crate::prim::bprim::HdBprim + Send + Sync + 'static> HdBprimSync for BprimAdapter<T> {
    fn sync_dyn(
        &mut self,
        delegate: &dyn crate::prim::HdSceneDelegate,
        render_param: Option<&dyn crate::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        self.0.sync(delegate, render_param, dirty_bits);
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Blanket adapter: wraps any T: HdSprim into a HdSprimSync object.
pub struct SprimAdapter<T: crate::prim::sprim::HdSprim + Send + Sync + 'static>(pub T);

impl<T: crate::prim::sprim::HdSprim + Send + Sync + 'static> HdSprimSync for SprimAdapter<T> {
    fn sync_dyn(
        &mut self,
        delegate: &dyn crate::prim::HdSceneDelegate,
        render_param: Option<&dyn crate::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        self.0.sync(delegate, render_param, dirty_bits);
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Blanket adapter: wraps any T: HdRprim into a HdRprimSync object.
pub struct RprimAdapter<T: crate::prim::rprim::HdRprim + Send + Sync + 'static>(pub T);

impl<T: crate::prim::rprim::HdRprim + Send + Sync + 'static> HdRprimSync for RprimAdapter<T> {
    fn sync_dyn(
        &mut self,
        delegate: &dyn crate::prim::HdSceneDelegate,
        render_param: Option<&dyn crate::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
        repr_token: &Token,
    ) {
        if std::env::var_os("USD_PROFILE_SYNC").is_some() {
            eprintln!(
                "[RprimAdapter::sync_dyn] concrete_type={}",
                std::any::type_name::<T>()
            );
        }
        self.0.sync(delegate, render_param, dirty_bits, repr_token);
    }
    fn propagate_dirty_bits_dyn(&self, bits: HdDirtyBits) -> HdDirtyBits {
        self.0.propagate_rprim_dirty_bits(bits)
    }
    fn init_repr_dyn(&mut self, repr_token: &Token, dirty_bits: &mut HdDirtyBits) {
        self.0.init_repr(repr_token, dirty_bits);
    }
    fn can_skip_dirty_bit_propagation_and_sync_dyn(&self, bits: HdDirtyBits) -> bool {
        self.0.can_skip_dirty_bit_propagation_and_sync(bits)
    }
    fn set_repr_selector_dyn(&mut self, repr_sel: crate::prim::HdReprSelector) {
        self.0.update_repr_selector(repr_sel);
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

/// Type-erased bprim handle supporting direct sync dispatch.
pub type HdBprimHandle = Box<dyn HdBprimSync>;
/// Type-erased sprim handle supporting direct sync dispatch.
pub type HdSprimHandle = Box<dyn HdSprimSync>;
/// Type-erased rprim handle supporting direct sync dispatch.
pub type HdRprimHandle = Box<dyn HdRprimSync>;

/// Prim entry for bprims (buffer objects).
struct BprimEntry {
    /// Opaque handle for render_delegate destroy/get API.
    handle: HdPrimHandle,
    /// Direct sync handle — populated if render_delegate provides a HdBprimSync.
    /// When Some, render_index calls sync_dyn() directly (correct C++ parity).
    /// When None, falls back to render_delegate.sync_bprim() dispatch.
    sync_handle: Option<HdBprimHandle>,
    #[allow(dead_code)]
    scene_delegate_id: SdfPath,
    type_id: Token,
}

/// Prim entry for sprims (state objects).
struct SprimEntry {
    handle: HdPrimHandle,
    sync_handle: Option<HdSprimHandle>,
    #[allow(dead_code)]
    scene_delegate_id: SdfPath,
    type_id: Token,
}

/// Prim entry for rprims (renderable geometry).
struct RprimEntry {
    handle: HdPrimHandle,
    sync_handle: Option<HdRprimHandle>,
    #[allow(dead_code)]
    scene_delegate_id: SdfPath,
    type_id: Token,
    /// Index into rprim_prim_id_map (used for picking). Updated by _compact_prim_ids (P0-1).
    prim_id: i32,
}

/// Task entry storing the task and its scene delegate.
///
/// Matches C++ `_TaskInfo` struct in HdRenderIndex.
struct TaskEntry {
    /// Scene delegate providing data for this task (used during Sync).
    /// Stored as an Arc so multiple tasks can share the same delegate.
    /// None for tasks inserted without an explicit delegate (legacy path).
    scene_delegate: Option<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>>,

    /// The task itself
    task: HdTaskSharedPtr,
}

/// Instancer entry in the render index.
struct InstancerEntry {
    /// The instancer object
    instancer: HdInstancer,

    /// Scene delegate that owns this instancer
    #[allow(dead_code)]
    scene_delegate_id: SdfPath,
}

/// Notice batching context for nested Begin/End depth tracking.
///
/// Matches C++ `HdRenderIndex::_NoticeBatchingContext` (renderIndex.cpp:90-143).
/// Wraps a notice batching scene index and tracks nested batching depth.
/// When a batching scene index is attached, begin/end calls toggle its
/// batching state and flush pending notices on end.
struct NoticeBatchingContext {
    /// Nesting depth for Begin/End calls
    depth: u32,
    /// Display name for debug diagnostics
    display_name: String,
    /// Optional notice batching scene index that accumulates notices.
    /// Set via attach_scene_index(); when None only depth tracking is active.
    batching_si: Option<Arc<RwLock<HdNoticeBatchingSceneIndex>>>,
}

impl NoticeBatchingContext {
    fn new(display_name: impl Into<String>) -> Self {
        Self {
            depth: 0,
            display_name: display_name.into(),
            batching_si: None,
        }
    }

    /// Attach a HdNoticeBatchingSceneIndex so that begin/end calls actually
    /// toggle batching on that scene index (matching C++ Append()).
    #[allow(dead_code)]
    fn attach_scene_index(&mut self, si: Arc<RwLock<HdNoticeBatchingSceneIndex>>) {
        self.batching_si = Some(si);
    }

    /// Wrap `input` in a new HdNoticeBatchingSceneIndex, attach it, and
    /// return the wrapping scene index (matches C++ _NoticeBatchingContext::Append).
    fn append(&mut self, input: HdSceneIndexHandle) -> Arc<RwLock<HdNoticeBatchingSceneIndex>> {
        let batching = HdNoticeBatchingSceneIndex::new(input);
        self.batching_si = Some(batching.clone());
        batching
    }

    /// Increment nesting depth. Enables batching on the scene index at depth 0->1.
    fn begin_batching(&mut self) {
        self.depth += 1;
        if self.depth == 1 {
            if let Some(ref si) = self.batching_si {
                {
                    HdNoticeBatchingSceneIndex::set_batching_enabled_unlocked(si, true);
                }
            }
        }
    }

    /// Decrement nesting depth. Flushes and disables batching at depth 1->0.
    fn end_batching(&mut self) {
        if self.depth == 0 {
            eprintln!(
                "Warning: Imbalanced batch begin/end calls for {}",
                self.display_name
            );
            return;
        }
        self.depth -= 1;
        if self.depth == 0 {
            // Flush accumulated notices by disabling batching.
            // HdNoticeBatchingSceneIndex::set_batching_enabled(false) flushes internally.
            if let Some(ref si) = self.batching_si {
                {
                    HdNoticeBatchingSceneIndex::set_batching_enabled_unlocked(si, false);
                }
            }
        }
    }

    /// Check if currently batching notices.
    fn is_batching(&self) -> bool {
        self.depth > 0
    }
}

impl Drop for NoticeBatchingContext {
    fn drop(&mut self) {
        if self.depth != 0 {
            eprintln!(
                "Warning: Imbalanced batch begin/end calls for {} (depth={}).",
                self.display_name, self.depth
            );
        }
    }
}

/// Central registry for all scene primitives.
///
/// Manages the lifecycle of all prims and coordinates synchronization.
/// Owns the change tracker that drives the sync pipeline.
pub struct HdRenderIndex {
    /// Render delegate for creating prims
    render_delegate: HdRenderDelegateSharedPtr,

    /// Change tracker - drives dirty-bit-based sync pipeline
    tracker: HdChangeTracker,

    /// Registered rprims (renderable geometry)
    rprims: HashMap<SdfPath, RprimEntry>,

    /// Registered sprims (state objects)
    sprims: HashMap<SdfPath, SprimEntry>,

    /// Registered bprims (buffer objects)
    bprims: HashMap<SdfPath, BprimEntry>,

    /// Registered tasks (units of work in rendering pipeline)
    tasks: HashMap<SdfPath, TaskEntry>,

    /// Registered instancers (geometry instancing)
    instancers: HashMap<SdfPath, InstancerEntry>,

    /// Driver vector passed during construction
    #[allow(dead_code)]
    drivers: HdDriverVector,

    /// Optional instance name
    instance_name: String,

    /// Optional application name
    #[allow(dead_code)] // C++ API surface, used for diagnostics
    app_name: String,

    // ---- Scene Index Integration ----
    /// Legacy prim scene index for scene delegate emulation (front-end emulation).
    /// Created when scene index emulation is enabled and no terminal scene index
    /// is provided. Legacy InsertRprim/Sprim/Bprim calls forward here.
    emulation_scene_index: Option<Arc<RwLock<HdLegacyPrimSceneIndex>>>,
    /// Terminal scene index driving prim population (Hydra 2.0)
    terminal_scene_index: Option<HdSceneIndexHandle>,
    /// Back-end emulation delegate mirroring the terminal scene index into the
    /// legacy HdSceneDelegate API, matching OpenUSD's
    /// `HdSceneIndexAdapterSceneDelegate` ownership model.
    scene_index_adapter_scene_delegate: Option<Arc<HdSceneIndexAdapterSceneDelegate>>,

    /// Merging scene index combining multiple input scenes
    merging_scene_index: Option<Arc<RwLock<HdMergingSceneIndex>>>,

    /// Collections queued for sync (from EnqueueCollectionToSync)
    collections_to_sync: Vec<HdRprimCollection>,

    /// Whether scene index emulation is enabled
    scene_index_emulation_enabled: bool,

    /// Fallback sprims created by the render delegate (one per type)
    fallback_sprims: HashMap<Token, HdPrimHandle>,

    /// Fallback bprims created by the render delegate (one per type)
    fallback_bprims: HashMap<Token, HdPrimHandle>,

    /// Rprim prim-ID-to-path map for picking/selection (index = primId).
    /// Matches C++ `_rprimPrimIdMap` (Vec, not HashMap) for O(1) lookup.
    rprim_prim_id_map: Vec<SdfPath>,

    /// Per-rprim render tag (defaults to "geometry")
    render_tags: HashMap<SdfPath, Token>,

    /// Per-rprim instancer association (for GetSceneDelegateAndInstancerIds)
    rprim_instancer_ids: HashMap<SdfPath, SdfPath>,

    /// Notice batching for emulation scene index.
    /// Matches C++ `_emulationBatchingCtx`.
    emulation_batching_ctx: NoticeBatchingContext,

    /// Notice batching for merging scene index.
    /// Matches C++ `_mergingBatchingCtx`.
    merging_batching_ctx: NoticeBatchingContext,

    /// Acceleration structure: set of known-dirty rprim paths.
    ///
    /// Matches C++ `HdDirtyList _rprimDirtyList`. Populated when rprims are
    /// marked dirty (insert / explicit mark), consumed during `sync_rprims`
    /// to avoid O(n) full scans of all rprims.
    dirty_rprim_ids: BTreeSet<SdfPath>,

    // --- Incremental rprim sync state ---
    /// Cached dirty rprim list for incremental sync (None = not started).
    incremental_dirty: Option<Vec<(SdfPath, HdDirtyBits)>>,
    /// Current offset into incremental_dirty.
    incremental_offset: usize,
    /// Repr token determined at start of incremental sync.
    incremental_repr_token: Option<Token>,
    /// Stats from initial dirty list collection (for pruning heuristics).
    incremental_candidates_len: usize,
    incremental_num_skipped: usize,
    incremental_num_non_varying: usize,
}

impl HdRenderIndex {
    /// Create a new render index with the given render delegate.
    ///
    /// # Parameters
    /// - `render_delegate` - Backend-specific render delegate
    /// - `drivers` - GPU device handles and contexts
    /// - `instance_name` - Optional identifier for this index
    ///
    /// # Returns
    /// New render index or None if render_delegate is invalid.
    pub fn new(
        render_delegate: HdRenderDelegateSharedPtr,
        drivers: HdDriverVector,
        instance_name: Option<String>,
        app_name: Option<String>,
    ) -> Option<Self> {
        // Set drivers on the render delegate
        render_delegate.write().set_drivers(&drivers);

        let mut tracker = HdChangeTracker::new();
        // Register well-known collection (matches C++ _tracker.AddCollection(HdTokens->geometry))
        tracker.add_collection(&Token::new("geometry"));

        let mut index = Self {
            render_delegate,
            tracker,
            rprims: HashMap::new(),
            sprims: HashMap::new(),
            bprims: HashMap::new(),
            tasks: HashMap::new(),
            instancers: HashMap::new(),
            drivers,
            instance_name: instance_name.unwrap_or_default(),
            app_name: app_name.unwrap_or_default(),
            emulation_scene_index: None,
            terminal_scene_index: None,
            scene_index_adapter_scene_delegate: None,
            merging_scene_index: None,
            collections_to_sync: Vec::new(),
            scene_index_emulation_enabled: true,
            fallback_sprims: HashMap::new(),
            fallback_bprims: HashMap::new(),
            rprim_prim_id_map: Vec::with_capacity(128),
            render_tags: HashMap::new(),
            rprim_instancer_ids: HashMap::new(),
            emulation_batching_ctx: NoticeBatchingContext::new("postEmulation"),
            merging_batching_ctx: NoticeBatchingContext::new("postMerging"),
            dirty_rprim_ids: BTreeSet::new(),
            incremental_dirty: None,
            incremental_offset: 0,
            incremental_repr_token: None,
            incremental_candidates_len: 0,
            incremental_num_skipped: 0,
            incremental_num_non_varying: 0,
        };

        // Register well-known repr descriptors once globally (C++ std::call_once(reprsOnce, _ConfigureReprs))
        static REPRS_ONCE: Once = Once::new();
        REPRS_ONCE.call_once(Self::_configure_reprs);

        // Create fallback prims (matches C++ _CreateFallbackPrims in ctor)
        index._create_fallback_prims();

        Some(index)
    }

    /// Create a new render index with a terminal scene index.
    ///
    /// Matches C++ `HdRenderIndex::New(delegate, drivers, terminalSceneIndex)`.
    /// Skips front-end emulation: the terminal scene index is set directly.
    pub fn new_with_terminal_scene_index(
        render_delegate: HdRenderDelegateSharedPtr,
        drivers: HdDriverVector,
        terminal_scene_index: HdSceneIndexHandle,
        instance_name: Option<String>,
    ) -> Option<Self> {
        let mut index = Self::new(render_delegate, drivers, instance_name, None)?;
        // Disable emulation since we have an explicit terminal scene index
        index.scene_index_emulation_enabled = false;
        index.set_terminal_scene_index(terminal_scene_index);
        Some(index)
    }

    //--------------------------------------------------------------------------
    // Change Tracker
    //--------------------------------------------------------------------------

    /// Get immutable reference to the change tracker.
    pub fn get_change_tracker(&self) -> &HdChangeTracker {
        &self.tracker
    }

    /// Get mutable reference to the change tracker.
    pub fn get_change_tracker_mut(&mut self) -> &mut HdChangeTracker {
        &mut self.tracker
    }

    //--------------------------------------------------------------------------
    // Rprim Operations
    //--------------------------------------------------------------------------

    /// Insert a renderable prim into the index.
    ///
    /// Creates the prim via the render delegate and registers it with the
    /// change tracker using initial dirty bits + DirtyRenderTag.
    ///
    /// Also forwards to the emulation scene index if emulation is enabled
    /// (Hydra 2.0 path: AddLegacyPrim on HdLegacyPrimSceneIndex).
    pub fn insert_rprim(
        &mut self,
        type_id: &Token,
        scene_delegate_id: &SdfPath,
        prim_id: &SdfPath,
    ) -> bool {
        if self.rprims.contains_key(prim_id) {
            return false;
        }

        let (handle, sync_handle) = {
            let mut delegate = self.render_delegate.write();
            let h = delegate.create_rprim(type_id, prim_id.clone());
            let sh = delegate.create_rprim_sync(type_id, prim_id);
            (h, sh)
        };

        if let Some(handle) = handle {
            // Register with tracker: AllDirty | DirtyRenderTag (matches C++)
            let initial_bits = crate::change_tracker::HdRprimDirtyBits::ALL_DIRTY
                | crate::change_tracker::HdRprimDirtyBits::DIRTY_RENDER_TAG;
            self.tracker.rprim_inserted(prim_id, initial_bits);

            // Allocate prim ID for picking (matches C++ _AllocatePrimId)
            self._allocate_prim_id(prim_id);

            // Track as dirty for accelerated sync
            self.dirty_rprim_ids.insert(prim_id.clone());

            let assigned_prim_id = (self.rprim_prim_id_map.len() - 1) as i32;
            self.rprims.insert(
                prim_id.clone(),
                RprimEntry {
                    handle,
                    sync_handle,
                    scene_delegate_id: scene_delegate_id.clone(),
                    type_id: type_id.clone(),
                    prim_id: assigned_prim_id,
                },
            );
            true
        } else {
            false
        }
    }

    /// Insert a renderable prim with an associated scene delegate.
    ///
    /// When emulation is enabled, forwards to HdLegacyPrimSceneIndex::add_legacy_prim
    /// so the Hydra 2.0 scene index pipeline sees the prim. Otherwise falls back
    /// to the standard insert_rprim path.
    ///
    /// Matches C++ `HdRenderIndex::InsertRprim(typeId, sceneDelegate, rprimId)`.
    pub fn insert_rprim_with_delegate(
        &mut self,
        type_id: &Token,
        scene_delegate: Option<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>>,
        prim_id: &SdfPath,
    ) -> bool {
        let delegate_id = scene_delegate
            .as_ref()
            .map(|d| d.get_delegate_id())
            .unwrap_or_else(SdfPath::absolute_root);

        if self.scene_index_emulation_enabled {
            // Ensure emulation scene index exists
            self.ensure_emulation_scene_index();
            if let Some(ref esi) = self.emulation_scene_index {
                esi.write()
                    .add_legacy_prim(prim_id.clone(), type_id.clone(), scene_delegate);
                return true;
            }
        }
        self.insert_rprim(type_id, &delegate_id, prim_id)
    }

    /// Remove a renderable prim from the index.
    pub fn remove_rprim(&mut self, prim_id: &SdfPath) -> bool {
        if let Some(entry) = self.rprims.remove(prim_id) {
            self.tracker.rprim_removed(prim_id);
            self.render_tags.remove(prim_id);
            self.rprim_instancer_ids.remove(prim_id);
            self.dirty_rprim_ids.remove(prim_id);
            // P1-1: clear the prim ID slot in the map to avoid stale picks.
            // Use empty path as tombstone (matches C++ HdRenderIndex::RemoveRprim).
            let id = entry.prim_id as usize;
            if id < self.rprim_prim_id_map.len() {
                self.rprim_prim_id_map[id] = SdfPath::empty();
            }
            let mut delegate = self.render_delegate.write();
            delegate.destroy_rprim(entry.handle);
            true
        } else {
            false
        }
    }

    /// Check if an rprim exists.
    pub fn has_rprim(&self, id: &SdfPath) -> bool {
        self.rprims.contains_key(id)
    }

    /// Get rprim type id.
    pub fn get_rprim_type_id(&self, id: &SdfPath) -> Option<&Token> {
        self.rprims.get(id).map(|entry| &entry.type_id)
    }

    /// Get mutable reference to rprim handle for syncing.
    pub fn get_rprim_handle_mut(&mut self, id: &SdfPath) -> Option<&mut HdPrimHandle> {
        self.rprims.get_mut(id).map(|entry| &mut entry.handle)
    }

    /// Get mutable reference to typed rprim sync handle (if present).
    ///
    /// When `create_rprim_sync` returned `Some`, the rprim is synced through
    /// this handle. Callers that need the post-sync typed state (e.g. to read
    /// draw items or world transform) must use this instead of `get_rprim_handle_mut`.
    pub fn get_rprim_sync_handle_mut(&mut self, id: &SdfPath) -> Option<&mut HdRprimHandle> {
        self.rprims
            .get_mut(id)
            .and_then(|entry| entry.sync_handle.as_mut())
    }

    //--------------------------------------------------------------------------
    // Sprim Operations
    //--------------------------------------------------------------------------

    /// Insert a state prim into the index.
    ///
    /// Registers with the change tracker on insertion.
    pub fn insert_sprim(
        &mut self,
        type_id: &Token,
        scene_delegate_id: &SdfPath,
        prim_id: &SdfPath,
    ) -> bool {
        if self.sprims.contains_key(prim_id) {
            return false;
        }

        let (handle, sync_handle) = {
            let mut delegate = self.render_delegate.write();
            let h = delegate.create_sprim(type_id, prim_id.clone());
            let sh = delegate.create_sprim_sync(type_id, prim_id);
            (h, sh)
        };

        if let Some(handle) = handle {
            let initial_bits = crate::change_tracker::HdRprimDirtyBits::ALL_DIRTY;
            self.tracker.sprim_inserted(prim_id, initial_bits);

            self.sprims.insert(
                prim_id.clone(),
                SprimEntry {
                    handle,
                    sync_handle,
                    scene_delegate_id: scene_delegate_id.clone(),
                    type_id: type_id.clone(),
                },
            );
            true
        } else {
            false
        }
    }

    /// Insert a state prim with an associated scene delegate.
    ///
    /// When emulation is enabled, forwards to HdLegacyPrimSceneIndex::add_legacy_prim.
    /// Matches C++ `HdRenderIndex::InsertSprim(typeId, sceneDelegate, sprimId)`.
    pub fn insert_sprim_with_delegate(
        &mut self,
        type_id: &Token,
        scene_delegate: Option<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>>,
        prim_id: &SdfPath,
    ) -> bool {
        let delegate_id = scene_delegate
            .as_ref()
            .map(|d| d.get_delegate_id())
            .unwrap_or_else(SdfPath::absolute_root);

        if self.scene_index_emulation_enabled {
            self.ensure_emulation_scene_index();
            if let Some(ref esi) = self.emulation_scene_index {
                esi.write()
                    .add_legacy_prim(prim_id.clone(), type_id.clone(), scene_delegate);
                return true;
            }
        }
        self.insert_sprim(type_id, &delegate_id, prim_id)
    }

    /// Remove a state prim from the index.
    ///
    /// Matches C++ `HdRenderIndex::RemoveSprim(TfToken const& typeId, SdfPath const& id)`.
    pub fn remove_sprim(&mut self, _type_id: &Token, prim_id: &SdfPath) -> bool {
        // _type_id used for type-keyed index lookup in C++ (Hd_PrimTypeIndex);
        // our HashMap is path-keyed so we don't need it yet, but keep for API parity.
        if let Some(entry) = self.sprims.remove(prim_id) {
            self.tracker.sprim_removed(prim_id);
            // Also clean up any sprim-sprim dependencies
            self.tracker
                .remove_sprim_from_sprim_sprim_dependencies(prim_id);
            let mut delegate = self.render_delegate.write();
            delegate.destroy_sprim(entry.handle);
            true
        } else {
            false
        }
    }

    /// Check if an sprim exists.
    pub fn has_sprim(&self, id: &SdfPath) -> bool {
        self.sprims.contains_key(id)
    }

    /// Get sprim type id.
    pub fn get_sprim_type_id(&self, id: &SdfPath) -> Option<&Token> {
        self.sprims.get(id).map(|entry| &entry.type_id)
    }

    //--------------------------------------------------------------------------
    // Bprim Operations
    //--------------------------------------------------------------------------

    /// Insert a buffer prim into the index.
    ///
    /// Registers with the change tracker on insertion.
    pub fn insert_bprim(
        &mut self,
        type_id: &Token,
        scene_delegate_id: &SdfPath,
        prim_id: &SdfPath,
    ) -> bool {
        if self.bprims.contains_key(prim_id) {
            return false;
        }

        let (handle, sync_handle) = {
            let mut delegate = self.render_delegate.write();
            let h = delegate.create_bprim(type_id, prim_id.clone());
            let sh = delegate.create_bprim_sync(type_id, prim_id);
            (h, sh)
        };

        if let Some(handle) = handle {
            let initial_bits = crate::change_tracker::HdRprimDirtyBits::ALL_DIRTY;
            self.tracker.bprim_inserted(prim_id, initial_bits);

            self.bprims.insert(
                prim_id.clone(),
                BprimEntry {
                    handle,
                    sync_handle,
                    scene_delegate_id: scene_delegate_id.clone(),
                    type_id: type_id.clone(),
                },
            );
            true
        } else {
            false
        }
    }

    /// Insert a buffer prim with an associated scene delegate.
    ///
    /// When emulation is enabled, forwards to HdLegacyPrimSceneIndex::add_legacy_prim.
    /// Matches C++ `HdRenderIndex::InsertBprim(typeId, sceneDelegate, bprimId)`.
    pub fn insert_bprim_with_delegate(
        &mut self,
        type_id: &Token,
        scene_delegate: Option<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>>,
        prim_id: &SdfPath,
    ) -> bool {
        let delegate_id = scene_delegate
            .as_ref()
            .map(|d| d.get_delegate_id())
            .unwrap_or_else(SdfPath::absolute_root);

        if self.scene_index_emulation_enabled {
            self.ensure_emulation_scene_index();
            if let Some(ref esi) = self.emulation_scene_index {
                esi.write()
                    .add_legacy_prim(prim_id.clone(), type_id.clone(), scene_delegate);
                return true;
            }
        }
        self.insert_bprim(type_id, &delegate_id, prim_id)
    }

    /// Remove a buffer prim from the index.
    ///
    /// Matches C++ `HdRenderIndex::RemoveBprim(TfToken const& typeId, SdfPath const& id)`.
    pub fn remove_bprim(&mut self, _type_id: &Token, prim_id: &SdfPath) -> bool {
        // _type_id used for type-keyed index lookup in C++ (Hd_PrimTypeIndex);
        // our HashMap is path-keyed so we don't need it yet, but keep for API parity.
        if let Some(entry) = self.bprims.remove(prim_id) {
            self.tracker.bprim_removed(prim_id);
            let mut delegate = self.render_delegate.write();
            delegate.destroy_bprim(entry.handle);
            true
        } else {
            false
        }
    }

    /// Check if a bprim exists.
    pub fn has_bprim(&self, id: &SdfPath) -> bool {
        self.bprims.contains_key(id)
    }

    /// Get bprim type id.
    pub fn get_bprim_type_id(&self, id: &SdfPath) -> Option<&Token> {
        self.bprims.get(id).map(|entry| &entry.type_id)
    }

    //--------------------------------------------------------------------------
    // Task Operations
    //--------------------------------------------------------------------------

    /// Insert a task into the index.
    ///
    /// Registers with the change tracker for dirty bit management.
    /// The scene delegate is stored and used during task Sync calls
    /// (matches C++ `_taskInfo.sceneDelegate`).
    pub fn insert_task(
        &mut self,
        scene_delegate: Option<Arc<dyn crate::prim::HdSceneDelegate + Send + Sync>>,
        task_id: &SdfPath,
        task: HdTaskSharedPtr,
    ) -> bool {
        if self.tasks.contains_key(task_id) {
            return false;
        }

        // Register with tracker (matches C++ _tracker.TaskInserted)
        let initial_bits = crate::change_tracker::HdTaskDirtyBits::ALL_DIRTY;
        self.tracker.task_inserted(task_id, initial_bits);

        self.tasks.insert(
            task_id.clone(),
            TaskEntry {
                scene_delegate,
                task,
            },
        );
        true
    }

    /// Remove a task from the index.
    pub fn remove_task(&mut self, task_id: &SdfPath) -> bool {
        if self.tasks.remove(task_id).is_some() {
            self.tracker.task_removed(task_id);
            true
        } else {
            false
        }
    }

    /// Check if a task exists in the index.
    pub fn has_task(&self, id: &SdfPath) -> bool {
        self.tasks.contains_key(id)
    }

    /// Get a task by path.
    pub fn get_task(&self, id: &SdfPath) -> Option<&HdTaskSharedPtr> {
        self.tasks.get(id).map(|entry| &entry.task)
    }

    /// Get all task ids.
    pub fn get_task_ids(&self) -> Vec<SdfPath> {
        self.tasks.keys().cloned().collect()
    }

    /// Get number of tasks.
    pub fn get_task_count(&self) -> usize {
        self.tasks.len()
    }

    //--------------------------------------------------------------------------
    // Instancer Operations
    //--------------------------------------------------------------------------

    /// Insert an instancer into the index.
    ///
    /// Creates the instancer and registers it with the change tracker.
    /// Matches C++ HdRenderIndex::_InsertInstancer.
    pub fn insert_instancer(
        &mut self,
        scene_delegate_id: &SdfPath,
        instancer_id: &SdfPath,
    ) -> bool {
        if self.instancers.contains_key(instancer_id) {
            return false;
        }

        let instancer = HdInstancer::new(None, instancer_id.clone(), None);
        let initial_bits = HdInstancer::get_initial_dirty_bits_mask();
        self.tracker.instancer_inserted(instancer_id, initial_bits);

        self.instancers.insert(
            instancer_id.clone(),
            InstancerEntry {
                instancer,
                scene_delegate_id: scene_delegate_id.clone(),
            },
        );
        true
    }

    /// Insert an instancer with a parent (nested instancing).
    pub fn insert_instancer_with_parent(
        &mut self,
        scene_delegate_id: &SdfPath,
        instancer_id: &SdfPath,
        parent_id: &SdfPath,
    ) -> bool {
        if self.instancers.contains_key(instancer_id) {
            return false;
        }

        let instancer = HdInstancer::new(None, instancer_id.clone(), Some(parent_id.clone()));
        let initial_bits = HdInstancer::get_initial_dirty_bits_mask();
        self.tracker.instancer_inserted(instancer_id, initial_bits);

        // Track parent-child dependency
        if !parent_id.is_empty() {
            self.tracker
                .add_instancer_instancer_dependency(parent_id, instancer_id);
        }

        self.instancers.insert(
            instancer_id.clone(),
            InstancerEntry {
                instancer,
                scene_delegate_id: scene_delegate_id.clone(),
            },
        );
        true
    }

    /// Remove an instancer from the index.
    ///
    /// Cleans up parent-child dependencies and notifies the change tracker.
    /// Matches C++ HdRenderIndex::_RemoveInstancer.
    pub fn remove_instancer(&mut self, instancer_id: &SdfPath) -> bool {
        if let Some(entry) = self.instancers.remove(instancer_id) {
            // Remove parent dependency if nested.
            let parent_id = entry.instancer.get_parent_id();
            if !parent_id.is_empty() {
                self.tracker
                    .remove_instancer_instancer_dependency(parent_id, instancer_id);
            }

            self.tracker.instancer_removed(instancer_id);
            true
        } else {
            false
        }
    }

    /// Check if an instancer exists.
    pub fn has_instancer(&self, id: &SdfPath) -> bool {
        self.instancers.contains_key(id)
    }

    /// Get an instancer by path.
    pub fn get_instancer(&self, id: &SdfPath) -> Option<&HdInstancer> {
        self.instancers.get(id).map(|entry| &entry.instancer)
    }

    /// Get a mutable instancer by path.
    pub fn get_instancer_mut(&mut self, id: &SdfPath) -> Option<&mut HdInstancer> {
        self.instancers
            .get_mut(id)
            .map(|entry| &mut entry.instancer)
    }

    /// Get all instancer ids.
    pub fn get_instancer_ids(&self) -> Vec<SdfPath> {
        self.instancers.keys().cloned().collect()
    }

    /// Get number of instancers.
    pub fn get_instancer_count(&self) -> usize {
        self.instancers.len()
    }

    //--------------------------------------------------------------------------
    // Scene Index Integration
    //--------------------------------------------------------------------------

    /// Insert a scene index into the merging scene index.
    ///
    /// Matches C++ HdRenderIndex::InsertSceneIndex.
    /// If needsPrefixing and scenePathPrefix != "/", wraps in HdPrefixingSceneIndex.
    pub fn insert_scene_index(
        &mut self,
        input_scene: &HdSceneIndexHandle,
        scene_path_prefix: &SdfPath,
        needs_prefixing: bool,
    ) {
        if !self.scene_index_emulation_enabled {
            eprintln!(
                "Warning: Unable to add scene index at prefix {} because emulation is off.",
                scene_path_prefix
            );
            return;
        }

        // Create merging scene index lazily if needed
        if self.merging_scene_index.is_none() {
            self.merging_scene_index = Some(HdMergingSceneIndex::new());
        }

        let resolved_scene: HdSceneIndexHandle = if needs_prefixing
            && !scene_path_prefix.is_empty()
            && scene_path_prefix.as_str() != "/"
        {
            // Wrap in prefixing scene index (returns Arc<RwLock<HdPrefixingSceneIndex>>)
            HdPrefixingSceneIndex::new(Some(input_scene.clone()), scene_path_prefix.clone())
        } else {
            input_scene.clone()
        };

        if let Some(ref merging) = self.merging_scene_index {
            let m = merging.write();
            m.add_input_scene(resolved_scene, scene_path_prefix.clone());
        }
    }

    /// Remove a scene index from the merging scene index.
    ///
    /// Matches C++ HdRenderIndex::RemoveSceneIndex.
    /// Handles both direct and prefixed scenes.
    pub fn remove_scene_index(&mut self, input_scene: &HdSceneIndexHandle) {
        if !self.scene_index_emulation_enabled {
            return;
        }

        if let Some(ref merging) = self.merging_scene_index {
            let m = merging.write();
            m.remove_input_scene(input_scene);
        }
    }

    /// Get the terminal scene index.
    pub fn get_terminal_scene_index(&self) -> Option<&HdSceneIndexHandle> {
        self.terminal_scene_index.as_ref()
    }

    /// Get the scene-index-backed delegate used for Hydra back-end emulation.
    ///
    /// This matches OpenUSD's `HdRenderIndex` ownership of
    /// `HdSceneIndexAdapterSceneDelegate` when a terminal scene index is present.
    pub fn get_scene_index_adapter_scene_delegate(
        &self,
    ) -> Option<Arc<HdSceneIndexAdapterSceneDelegate>> {
        self.scene_index_adapter_scene_delegate.clone()
    }

    /// Set the terminal scene index.
    ///
    /// Also wires the canonical Hydra back-end emulation delegate and then
    /// notifies the render delegate via `set_terminal_scene_index`.
    pub fn set_terminal_scene_index(&mut self, scene_index: HdSceneIndexHandle) {
        log::info!("[render_index] set_terminal_scene_index: creating adapter");
        let render_index_ptr = std::ptr::NonNull::from(&mut *self);
        let adapter = Arc::new(HdSceneIndexAdapterSceneDelegate::new(
            scene_index.clone(),
            SdfPath::absolute_root(),
            Some(render_index_ptr),
        ));
        let observer = adapter.clone() as HdSceneIndexObserverHandle;
        log::info!("[render_index] adding adapter as observer on terminal SI");
        scene_index.read().add_observer(observer);
        log::info!("[render_index] adapter observer added");

        self.terminal_scene_index = Some(scene_index.clone());
        self.scene_index_adapter_scene_delegate = Some(adapter);

        {
            let mut delegate = self.render_delegate.write();
            delegate.set_terminal_scene_index(scene_index.clone());
        }
        // Initial traversal: walk the terminal scene index and send PrimsAdded
        // for all existing prims. This handles the case where populate() ran
        // before the observer was attached (set_stage before chaining).
        {
            let si_read = scene_index.read();
            let mut entries = Vec::new();
            Self::collect_existing_prims(&*si_read, &SdfPath::absolute_root(), &mut entries);
            if !entries.is_empty() {
                log::info!("[render_index] initial traversal: {} prims", entries.len());
                use crate::scene_index::HdSceneIndexObserver;
                drop(si_read); // release read lock before adapter callback
                self.scene_index_adapter_scene_delegate
                    .as_ref()
                    .unwrap()
                    .prims_added(&NullSceneIndex, &entries);
            }
        }
    }
}

/// Dummy scene index used as sender for initial traversal PrimsAdded.
struct NullSceneIndex;
impl crate::scene_index::HdSceneIndexBase for NullSceneIndex {
    fn get_prim(&self, _: &SdfPath) -> crate::scene_index::HdSceneIndexPrim {
        crate::scene_index::HdSceneIndexPrim::default()
    }
    fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
        Vec::new()
    }
    fn add_observer(&self, _: crate::scene_index::HdSceneIndexObserverHandle) {}
    fn remove_observer(&self, _: &crate::scene_index::HdSceneIndexObserverHandle) {}
    fn _system_message(&self, _: &usd_tf::Token, _: Option<crate::HdDataSourceBaseHandle>) {}
    fn get_display_name(&self) -> String {
        "NullSceneIndex".into()
    }
}

impl HdRenderIndex {
    /// Recursively collect all existing prims from a scene index.
    fn collect_existing_prims(
        si: &dyn crate::scene_index::HdSceneIndexBase,
        path: &SdfPath,
        entries: &mut Vec<crate::scene_index::AddedPrimEntry>,
    ) {
        let prim = si.get_prim(path);
        let child_paths = si.get_child_prim_paths(path);
        entries.push(crate::scene_index::AddedPrimEntry {
            prim_path: path.clone(),
            prim_type: prim.prim_type,
            data_source: prim.data_source,
        });
        for child_path in child_paths {
            Self::collect_existing_prims(si, &child_path, entries);
        }
    }

    /// Check if scene index emulation is enabled.
    ///
    /// Matches C++ `static bool HdRenderIndex::IsSceneIndexEmulationEnabled()`.
    /// Reads from `HD_ENABLE_SCENE_INDEX_EMULATION` env var (default: true).
    pub fn is_scene_index_emulation_enabled() -> bool {
        use std::sync::OnceLock;
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("HD_ENABLE_SCENE_INDEX_EMULATION")
                .map(|v| v != "0" && v.to_lowercase() != "false")
                .unwrap_or(true)
        })
    }

    /// Enable/disable scene index emulation.
    pub fn set_scene_index_emulation_enabled(&mut self, enabled: bool) {
        self.scene_index_emulation_enabled = enabled;
    }

    /// Initialize scene index emulation if not already set up.
    ///
    /// Creates an HdLegacyPrimSceneIndex, wraps it in a notice batching scene
    /// index (via emulation_batching_ctx.append), then adds it to the merging
    /// scene index. This is called lazily on first legacy prim insertion.
    ///
    /// Matches C++ HdRenderIndex constructor emulation setup:
    /// `_emulationSceneIndex = HdLegacyPrimSceneIndex::New()`
    /// `_emulationBatchingCtx->Append(_emulationSceneIndex)` -> batching SI
    /// `_mergingSceneIndex->AddInputScene(batchingSI, root)`
    pub fn ensure_emulation_scene_index(&mut self) {
        if !self.scene_index_emulation_enabled {
            return;
        }
        if self.emulation_scene_index.is_some() {
            return;
        }

        // Create the legacy prim scene index
        let emulation_si = HdLegacyPrimSceneIndex::new();
        self.emulation_scene_index = Some(emulation_si.clone());

        // Wrap the emulation scene index in a notice batching scene index
        // (matches C++ _emulationBatchingCtx->Append(_emulationSceneIndex)).
        // The batching SI is the downstream end that emulates prim population;
        // we feed this into the merging scene index.
        let batching_si: HdSceneIndexHandle = self
            .emulation_batching_ctx
            .append(emulation_si as HdSceneIndexHandle);

        // Create merging scene index if needed and add the batching SI
        if self.merging_scene_index.is_none() {
            self.merging_scene_index = Some(HdMergingSceneIndex::new());
        }
        if let Some(ref merging) = self.merging_scene_index {
            let m = merging.write();
            m.add_input_scene(batching_si, SdfPath::absolute_root());
        }
    }

    /// Get the emulation (legacy prim) scene index, if created.
    pub fn get_emulation_scene_index(&self) -> Option<&Arc<RwLock<HdLegacyPrimSceneIndex>>> {
        self.emulation_scene_index.as_ref()
    }

    //--------------------------------------------------------------------------
    // Collection Sync
    //--------------------------------------------------------------------------

    /// Queue a collection for sync during the next SyncAll call.
    ///
    /// Matches C++ HdRenderIndex::EnqueueCollectionToSync.
    /// Tasks call this during their Sync phase to specify which
    /// collections need rprim syncing.
    pub fn enqueue_collection_to_sync(&mut self, collection: HdRprimCollection) {
        self.collections_to_sync.push(collection);
    }

    //--------------------------------------------------------------------------
    // Synchronization - Multi-phase SyncAll pipeline
    //--------------------------------------------------------------------------

    /// Sync all dirty prims.
    ///
    /// This is the main sync entry point matching C++ HdRenderIndex::SyncAll.
    ///
    /// # Phases
    ///
    /// 1. **Render delegate Update** - for scene index emulation
    /// 2. **Bprim sync** - sync dirty buffer prims via tracker
    /// 3. **Sprim sync** - sync dirty state prims via tracker
    /// 4. **Task sync** - sync tasks using dirty bits from tracker
    /// 5. **Rprim sync** - collect dirty rprims, sync instancers, sync rprims
    /// 6. **Clean up** - clear collections, reset varying state
    /// 7. **Commit resources** - tell render delegate to commit
    ///
    /// # Parameters
    /// - `tasks` - Tasks to sync and execute
    /// - `task_context` - Shared state for inter-task communication
    pub fn sync_all(&mut self, tasks: &mut [HdTaskSharedPtr], task_context: &mut HdTaskContext) {
        let diag_sync = std::env::var_os("USD_PROFILE_SYNC").is_some();
        let diag = |msg: &str| {
            if diag_sync {
                eprintln!("[render_index::sync_all] {msg}");
            }
        };
        // Phase 1: Render delegate Update (scene index emulation)
        if self.scene_index_emulation_enabled {
            diag("delegate.update");
            let mut delegate = self.render_delegate.write();
            delegate.update();
        }

        let had_dirty_sprims = self.has_dirty_sprims();

        // Clone the Arc to break the borrow on `self` before calling &mut self methods.
        let adapter_arc = self.scene_index_adapter_scene_delegate.clone();
        if let Some(ref adapter) = adapter_arc {
            // SAFETY: HdSceneIndexAdapterSceneDelegate uses interior mutability (RwLock/Mutex)
            // for all real state changes. The &mut requirement comes from HdSceneDelegate trait
            // contract only. Arc::as_ptr gives a raw pointer independent of any shared reference.
            #[allow(unsafe_code)]
            let adapter_mut: &mut HdSceneIndexAdapterSceneDelegate =
                unsafe { &mut *Arc::as_ptr(adapter).cast_mut() };
            diag("sync_bprims");
            self.sync_bprims_impl(adapter_mut);
            diag("sync_sprims");
            self.sync_sprims_impl(adapter_mut);
            diag("sync_tasks");
            self.sync_tasks_with(tasks, task_context, adapter_mut);
            diag("sync_rprims");
            self.sync_rprims_with_delegate_mut(adapter_mut);
            if had_dirty_sprims {
                diag("post_sync_cleanup");
                adapter_mut.post_sync_cleanup();
            }
        } else {
            diag("sync_bprims");
            self.sync_bprims();
            diag("sync_sprims");
            self.sync_sprims();
            diag("sync_tasks");
            self.sync_tasks(tasks, task_context);
            diag("sync_rprims");
            self.sync_rprims();
        }

        // Phase 6: Clean up
        diag("cleanup");
        self.collections_to_sync.clear();

        // NOTE: commit_resources() is NOT called here.
        // In C++, HdEngine::Execute() calls CommitResources() AFTER SyncAll().
        // The caller (HdEngine) is responsible for calling commit_resources().
    }

    /// Runs sync phases 1-4 (delegate update, bprims, sprims, tasks)
    /// without touching rprims. Use with `sync_rprims_incremental` for
    /// progressive rprim sync.
    pub fn sync_pre_rprims(
        &mut self,
        tasks: &mut [HdTaskSharedPtr],
        task_context: &mut HdTaskContext,
    ) {
        // Phase 1: Render delegate Update (scene index emulation)
        if self.scene_index_emulation_enabled {
            let mut delegate = self.render_delegate.write();
            delegate.update();
        }

        // Clone the Arc to break the borrow on `self` before calling &mut self methods.
        let adapter_arc = self.scene_index_adapter_scene_delegate.clone();
        if let Some(ref adapter) = adapter_arc {
            // SAFETY: HdSceneIndexAdapterSceneDelegate uses interior mutability (RwLock/Mutex)
            // for all real state changes. The &mut requirement comes from HdSceneDelegate trait
            // contract only. Arc::as_ptr gives a raw pointer independent of any shared reference.
            #[allow(unsafe_code)]
            let adapter_mut: &mut HdSceneIndexAdapterSceneDelegate =
                unsafe { &mut *Arc::as_ptr(adapter).cast_mut() };
            self.sync_bprims_impl(adapter_mut);
            self.sync_sprims_impl(adapter_mut);
            self.sync_tasks_with(tasks, task_context, adapter_mut);
        } else {
            self.sync_bprims();
            self.sync_sprims();
            self.sync_tasks(tasks, task_context);
        }
    }

    /// Incrementally syncs up to `budget` dirty rprims per call.
    ///
    /// Uses the scene_index_adapter_scene_delegate when available (scene index
    /// emulation path), falling back to NULL_SCENE_DELEGATE otherwise.
    ///
    /// On the first call (when `incremental_dirty` is None), collects the dirty
    /// list, syncs instancers, and determines the repr token. Each subsequent
    /// call syncs the next `budget` rprims. Returns the number of remaining
    /// rprims. When 0 is returned, the incremental sync is complete.
    pub fn sync_rprims_incremental(&mut self, budget: usize) -> usize {
        // Clone adapter Arc to avoid borrow conflicts with &mut self
        let adapter_arc = self.scene_index_adapter_scene_delegate.clone();

        // First call: collect dirty list, sync instancers, determine repr,
        // and call delegate.sync() with the FULL dirty list upfront so
        // delegate-backed data is ready before rprims pull it.
        if self.incremental_dirty.is_none() {
            let candidates: Vec<SdfPath> = self.dirty_rprim_ids.iter().cloned().collect();
            let mut dirty = Vec::with_capacity(candidates.len());
            let mut num_skipped: usize = 0;
            let mut num_non_varying: usize = 0;

            for id in &candidates {
                let bits = self.tracker.get_rprim_dirty_bits(id);
                if !crate::change_tracker::HdRprimDirtyBits::is_varying(bits) {
                    num_non_varying += 1;
                }
                if crate::change_tracker::HdRprimDirtyBits::is_clean(bits) {
                    num_skipped += 1;
                } else {
                    dirty.push((id.clone(), bits));
                }
            }

            self.sync_instancers();

            let repr_token = self
                .collections_to_sync
                .first()
                .and_then(|c| {
                    let tok = c.get_repr_selector().get_token(0);
                    if tok.as_str().is_empty() {
                        None
                    } else {
                        Some(tok.clone())
                    }
                })
                .unwrap_or_else(|| Token::new("refined"));

            // Per C++ SyncAll: call delegate.sync() with aggregate request
            // BEFORE rprim sync so delegate-backed data is ready.
            if let Some(ref adapter) = adapter_arc {
                let mut aggregate = crate::HdSyncRequestVector::default();
                aggregate.ids.reserve(dirty.len());
                aggregate.dirty_bits.reserve(dirty.len());
                for (id, bits) in &dirty {
                    aggregate.ids.push(id.clone());
                    aggregate.dirty_bits.push(*bits);
                }
                adapter.sync(&mut aggregate);
            }

            log::trace!(
                "[PERF] sync_rprims_incremental: init candidates={} dirty={} skipped={}",
                candidates.len(),
                dirty.len(),
                num_skipped
            );

            self.incremental_candidates_len = candidates.len();
            self.incremental_num_skipped = num_skipped;
            self.incremental_num_non_varying = num_non_varying;
            self.incremental_repr_token = Some(repr_token);
            self.incremental_offset = 0;
            self.incremental_dirty = Some(dirty);
        }

        let total = self
            .incremental_dirty
            .as_ref()
            .map(|d| d.len())
            .unwrap_or(0);
        let offset = self.incremental_offset;
        let end = (offset + budget).min(total);

        // Clone the batch (paths + bits) so we can mutate self.rprims/tracker
        let batch: Vec<(SdfPath, HdDirtyBits)> = self
            .incremental_dirty
            .as_ref()
            .map(|d| d[offset..end].to_vec())
            .unwrap_or_default();

        let repr_token = self
            .incremental_repr_token
            .clone()
            .unwrap_or_else(|| Token::new("refined"));
        let repr_selector = crate::prim::HdReprSelector::with_token(repr_token.clone());
        let tracker = &mut self.tracker;

        // Sync each rprim in the batch using the scene index adapter when available
        if let Some(ref adapter) = adapter_arc {
            let adapter_ref: &HdSceneIndexAdapterSceneDelegate = adapter.as_ref();
            for (id, mut bits) in batch {
                if let Some(entry) = self.rprims.get_mut(&id) {
                    if let Some(ref mut sh) = entry.sync_handle {
                        let _ = Self::pre_sync_typed_rprim(
                            tracker,
                            &id,
                            sh,
                            &mut bits,
                            &repr_selector,
                            &repr_token,
                        );
                        sh.sync_dyn(adapter_ref, None, &mut bits, &repr_token);
                    } else {
                        let delegate = self.render_delegate.read();
                        delegate.sync_rprim(
                            &mut entry.handle,
                            &id,
                            adapter_ref,
                            &mut bits,
                            &repr_token,
                        );
                    }
                }
                let clean_bits = bits & crate::change_tracker::HdRprimDirtyBits::VARYING;
                tracker.mark_rprim_clean(&id, clean_bits);
            }
        } else {
            for (id, mut bits) in batch {
                if let Some(entry) = self.rprims.get_mut(&id) {
                    if let Some(ref mut sh) = entry.sync_handle {
                        let _ = Self::pre_sync_typed_rprim(
                            tracker,
                            &id,
                            sh,
                            &mut bits,
                            &repr_selector,
                            &repr_token,
                        );
                        sh.sync_dyn(&NULL_SCENE_DELEGATE, None, &mut bits, &repr_token);
                    } else {
                        let delegate = self.render_delegate.read();
                        delegate.sync_rprim(
                            &mut entry.handle,
                            &id,
                            &NULL_SCENE_DELEGATE,
                            &mut bits,
                            &repr_token,
                        );
                    }
                }
                let clean_bits = bits & crate::change_tracker::HdRprimDirtyBits::VARYING;
                tracker.mark_rprim_clean(&id, clean_bits);
            }
        }

        self.incremental_offset = end;
        let remaining = total - end;

        if remaining == 0 {
            // On completion with adapter: call post_sync_cleanup
            if let Some(ref adapter) = adapter_arc {
                adapter.post_sync_cleanup();
            }

            // Apply pruning heuristics (5g) then clear
            let num_candidates = self.incremental_candidates_len;
            let num_skipped = self.incremental_num_skipped;
            let num_non_varying = self.incremental_num_non_varying;

            const MIN_DIRTY_LIST_SIZE: usize = 500;
            const MIN_RATIO_SKIPPED: f32 = 0.25;
            const MIN_RATIO_NON_VARYING: f32 = 0.10;

            if num_candidates > MIN_DIRTY_LIST_SIZE {
                let ratio_skipped = num_skipped as f32 / num_candidates as f32;
                let ratio_non_varying = num_non_varying as f32 / num_candidates as f32;
                if ratio_skipped > MIN_RATIO_SKIPPED {
                    self.tracker.reset_varying_state();
                } else if ratio_non_varying > MIN_RATIO_NON_VARYING {
                    self.dirty_rprim_ids.retain(|id| {
                        let bits = self.tracker.get_rprim_dirty_bits(id);
                        crate::change_tracker::HdRprimDirtyBits::is_varying(bits)
                    });
                    self.incremental_dirty = None;
                    self.incremental_offset = 0;
                    self.incremental_repr_token = None;
                    self.collections_to_sync.clear();
                    return 0;
                }
            }

            self.dirty_rprim_ids.clear();
            self.incremental_dirty = None;
            self.incremental_offset = 0;
            self.incremental_repr_token = None;
            self.collections_to_sync.clear();
        }

        remaining
    }

    /// Reset incremental sync state (e.g. when mark_all_rprims_dirty is called).
    pub fn reset_incremental_sync(&mut self) {
        self.incremental_dirty = None;
        self.incremental_offset = 0;
        self.incremental_repr_token = None;
    }

    /// Returns total number of dirty rprims in the incremental list.
    pub fn incremental_dirty_count(&self) -> usize {
        self.incremental_dirty
            .as_ref()
            .map(|d| d.len())
            .unwrap_or(0)
    }

    /// Sync all dirty prims with a specific scene delegate.
    ///
    /// Like `sync_all` but uses the provided scene delegate for all data
    /// queries during sync, rather than the null delegate. This is the
    /// typical usage in a real rendering pipeline where UsdImagingDelegate
    /// or similar provides data.
    pub fn sync_all_with_delegate(
        &mut self,
        tasks: &mut [HdTaskSharedPtr],
        task_context: &mut HdTaskContext,
        scene_delegate: &dyn crate::prim::HdSceneDelegate,
    ) {
        // Phase 1: Render delegate Update (scene index emulation)
        if self.scene_index_emulation_enabled {
            let mut delegate = self.render_delegate.write();
            delegate.update();
        }

        // Phase 2-3: Sync bprims and sprims
        let bp_t0 = std::time::Instant::now();
        self.sync_bprims_with(scene_delegate);
        let bp_ms = bp_t0.elapsed().as_secs_f64() * 1000.0;
        self.sync_sprims_with(scene_delegate);
        let sp_ms = bp_t0.elapsed().as_secs_f64() * 1000.0 - bp_ms;

        // Phase 4: Task sync
        self.sync_tasks_with(tasks, task_context, scene_delegate);

        // Phase 5: Rprim sync
        let rp_t0 = std::time::Instant::now();
        self.sync_rprims_with(scene_delegate);
        let rp_ms = rp_t0.elapsed().as_secs_f64() * 1000.0;

        log::trace!(
            "[PERF]     sync_all phases: bprims={:.1}ms sprims={:.1}ms rprims={:.1}ms",
            bp_ms,
            sp_ms,
            rp_ms
        );

        // Phase 6: Clean up
        self.collections_to_sync.clear();
    }

    /// Phase 2: Sync dirty bprims.
    ///
    /// Iterates all tracked bprims, checks dirty bits, calls bprim.Sync()
    /// via the render delegate, and marks clean.
    /// Matches C++ renderIndex.cpp SyncAll bprim phase.
    fn sync_bprims(&mut self) {
        self.sync_bprims_impl(&NULL_SCENE_DELEGATE);
    }

    /// Phase 3: Sync dirty sprims.
    ///
    /// Iterates all tracked sprims, checks dirty bits, calls sprim.Sync()
    /// via the render delegate, and marks clean.
    /// Matches C++ renderIndex.cpp SyncAll sprim phase.
    fn sync_sprims(&mut self) {
        self.sync_sprims_impl(&NULL_SCENE_DELEGATE);
    }

    /// Phase 4: Sync tasks.
    ///
    /// Matches C++ task sync loop:
    /// - Tracked tasks: use their registered scene delegate; post-sync dirty
    ///   bits preserved via `mark_task_clean(id, dirty_bits)` (not zero).
    /// - Untracked tasks: dirty_bits = 0, synced with null delegate, no tracking.
    fn sync_tasks(&mut self, tasks: &mut [HdTaskSharedPtr], task_context: &mut HdTaskContext) {
        // Pre-collect (task_id, is_tracked, initial_dirty_bits) to avoid borrow conflicts.
        let task_infos: Vec<(SdfPath, bool, HdDirtyBits)> = tasks
            .iter()
            .map(|task| {
                let task_id = task.read().id().clone();
                if self.tasks.contains_key(&task_id) {
                    let bits = self.tracker.get_task_dirty_bits(&task_id);
                    (task_id, true, bits)
                } else {
                    // Untracked: dirty_bits = 0, no delegate (matches C++ `taskDirtyBits = 0`).
                    (task_id, false, 0)
                }
            })
            .collect();

        for (task, (task_id, is_tracked, mut dirty_bits)) in tasks.iter().zip(task_infos) {
            if is_tracked {
                // Tracked: use the task's registered scene delegate.
                let delegate_arc = self
                    .tasks
                    .get(&task_id)
                    .and_then(|e| e.scene_delegate.clone());

                if let Some(ref delegate) = delegate_arc {
                    task.write()
                        .sync(delegate.as_ref(), task_context, &mut dirty_bits);
                } else {
                    task.write()
                        .sync(&NULL_SCENE_DELEGATE, task_context, &mut dirty_bits);
                }

                // Preserve post-sync dirty bits (matches C++ MarkTaskClean(id, taskDirtyBits)).
                self.tracker.mark_task_clean(&task_id, dirty_bits);
            } else {
                // Untracked: sync with null delegate, no marking.
                task.write()
                    .sync(&NULL_SCENE_DELEGATE, task_context, &mut dirty_bits);
            }
        }
    }

    /// Phase 5: Sync rprims.
    ///
    /// Multi-step rprim sync matching C++ SyncAll rprim phases:
    /// a. Collect dirty rprim ids from dirty_rprim_ids acceleration structure
    /// b. Sync dirty instancers first (they affect rprim transforms)
    /// c. Call rprim.Sync() for each dirty rprim via render delegate
    /// d. Mark clean preserving Varying bit
    ///
    /// Matches C++ renderIndex.cpp SyncAll rprim phase.
    fn sync_rprims(&mut self) {
        self.sync_rprims_impl(&NULL_SCENE_DELEGATE);
    }

    /// Phase 2 (with delegate): Sync dirty bprims with a real scene delegate.
    fn sync_bprims_with(&mut self, scene_delegate: &dyn crate::prim::HdSceneDelegate) {
        self.sync_bprims_impl(scene_delegate);
    }

    /// Phase 3 (with delegate): Sync dirty sprims with a real scene delegate.
    fn sync_sprims_with(&mut self, scene_delegate: &dyn crate::prim::HdSceneDelegate) {
        self.sync_sprims_impl(scene_delegate);
    }

    /// Phase 4 (with delegate): Sync tasks with a fallback scene delegate.
    ///
    /// Each tracked task uses its own registered delegate; `scene_delegate` is
    /// used only for tracked tasks inserted without one. Untracked tasks get
    /// dirty_bits = 0 and the null delegate.
    fn sync_tasks_with(
        &mut self,
        tasks: &mut [HdTaskSharedPtr],
        task_context: &mut HdTaskContext,
        scene_delegate: &dyn crate::prim::HdSceneDelegate,
    ) {
        let task_infos: Vec<(SdfPath, bool, HdDirtyBits)> = tasks
            .iter()
            .map(|task| {
                let task_id = task.read().id().clone();
                if self.tasks.contains_key(&task_id) {
                    let bits = self.tracker.get_task_dirty_bits(&task_id);
                    (task_id, true, bits)
                } else {
                    (task_id, false, 0)
                }
            })
            .collect();

        for (task, (task_id, is_tracked, mut dirty_bits)) in tasks.iter().zip(task_infos) {
            if is_tracked {
                // Use the task's own registered delegate, fall back to provided delegate.
                let delegate_arc = self
                    .tasks
                    .get(&task_id)
                    .and_then(|e| e.scene_delegate.clone());

                if let Some(ref delegate) = delegate_arc {
                    task.write()
                        .sync(delegate.as_ref(), task_context, &mut dirty_bits);
                } else {
                    task.write()
                        .sync(scene_delegate, task_context, &mut dirty_bits);
                }

                // Preserve post-sync dirty bits.
                self.tracker.mark_task_clean(&task_id, dirty_bits);
            } else {
                task.write()
                    .sync(&NULL_SCENE_DELEGATE, task_context, &mut dirty_bits);
            }
        }
    }

    /// Phase 5 (with delegate): Sync rprims with a real scene delegate.
    fn sync_rprims_with(&mut self, scene_delegate: &dyn crate::prim::HdSceneDelegate) {
        self.sync_rprims_impl(scene_delegate);
    }

    // -----------------------------------------------------------------------
    // Consolidated sync impl methods (shared by null-delegate and with-delegate paths)
    // -----------------------------------------------------------------------

    /// Core bprim sync: iterate dirty bprims, call sync_dyn() or fallback,
    /// mark clean. Used by both sync_bprims() and sync_bprims_with().
    fn sync_bprims_impl(&mut self, scene_delegate: &dyn crate::prim::HdSceneDelegate) {
        // Collect dirty ids first to avoid borrow conflicts.
        let dirty: Vec<(SdfPath, HdDirtyBits)> = self
            .bprims
            .keys()
            .filter_map(|id| {
                let bits = self.tracker.get_bprim_dirty_bits(id);
                if bits != 0 {
                    Some((id.clone(), bits))
                } else {
                    None
                }
            })
            .collect();

        for (id, mut bits) in dirty {
            if let Some(entry) = self.bprims.get_mut(&id) {
                if let Some(ref mut sh) = entry.sync_handle {
                    // Direct sync via HdBprimSync — correct C++ parity.
                    // render_param not yet plumbed; pass None (matches C++ when no param).
                    sh.sync_dyn(scene_delegate, None, &mut bits);
                } else {
                    // Fallback: dispatch through render_delegate (backends that
                    // override sync_bprim() and do their own downcast).
                    let delegate = self.render_delegate.read();
                    delegate.sync_bprim(&mut entry.handle, &id, scene_delegate, &mut bits);
                }
            }
            self.tracker.mark_bprim_clean(&id, 0);
        }
    }

    /// Core sprim sync: iterate dirty sprims, call sync_dyn() or fallback,
    /// mark clean.
    fn sync_sprims_impl(&mut self, scene_delegate: &dyn crate::prim::HdSceneDelegate) {
        let dirty: Vec<(SdfPath, HdDirtyBits)> = self
            .sprims
            .keys()
            .filter_map(|id| {
                let bits = self.tracker.get_sprim_dirty_bits(id);
                if bits != 0 {
                    Some((id.clone(), bits))
                } else {
                    None
                }
            })
            .collect();

        for (id, mut bits) in dirty {
            if let Some(entry) = self.sprims.get_mut(&id) {
                if let Some(ref mut sh) = entry.sync_handle {
                    sh.sync_dyn(scene_delegate, None, &mut bits);
                } else {
                    let delegate = self.render_delegate.read();
                    delegate.sync_sprim(&mut entry.handle, &id, scene_delegate, &mut bits);
                }
            }
            self.tracker.mark_sprim_clean(&id, 0);
        }
    }

    /// Core rprim sync: collect dirty rprims, sync instancers, call sync_dyn()
    /// or fallback, mark clean preserving Varying bit.
    ///
    /// Also calls scene_delegate.sync(request) per delegate bucket and
    /// scene_delegate.post_sync_cleanup() after syncing, matching C++ SyncAll.
    fn sync_rprims_impl(&mut self, scene_delegate: &dyn crate::prim::HdSceneDelegate) {
        // 5a. Collect dirty rprim ids, tracking skip/non-varying counts for heuristics.
        // Matches C++ dirty list pruning logic (renderIndex.cpp:1740-1770).
        const MIN_DIRTY_LIST_SIZE: usize = 500;
        const MIN_RATIO_SKIPPED: f32 = 0.25;
        const MIN_RATIO_NON_VARYING: f32 = 0.10;

        let candidates: Vec<SdfPath> = self.dirty_rprim_ids.iter().cloned().collect();
        let num_candidates = candidates.len();
        let mut num_skipped: usize = 0;
        let mut num_non_varying: usize = 0;

        let mut dirty: Vec<(SdfPath, HdDirtyBits)> = Vec::with_capacity(candidates.len());
        for id in &candidates {
            let bits = self.tracker.get_rprim_dirty_bits(id);
            if !crate::change_tracker::HdRprimDirtyBits::is_varying(bits) {
                num_non_varying += 1;
            }
            if crate::change_tracker::HdRprimDirtyBits::is_clean(bits) {
                num_skipped += 1;
            } else {
                dirty.push((id.clone(), bits));
            }
        }

        // 5b. Sync dirty instancers first.
        self.sync_instancers();

        // 5c. Determine active repr token from queued collections.
        let repr_token = self
            .collections_to_sync
            .first()
            .and_then(|c| {
                let tok = c.get_repr_selector().get_token(0);
                if tok.as_str().is_empty() {
                    None
                } else {
                    Some(tok.clone())
                }
            })
            .unwrap_or_else(|| Token::new("refined"));
        let repr_selector = self
            .collections_to_sync
            .first()
            .map(|c| c.get_repr_selector().clone())
            .unwrap_or_else(|| crate::prim::HdReprSelector::with_token(repr_token.clone()));

        // P0-3: Build per-delegate sync request map and call delegate.sync() per bucket.
        // Key = scene_delegate_id, value = HdSyncRequestVector for that delegate.
        // Note: HdSceneDelegate::sync requires &mut self, so this &dyn path cannot
        // call it directly. Callers with &mut delegate should use sync_rprims_with_delegate_mut.
        let mut delegate_sync_map: std::collections::HashMap<SdfPath, crate::HdSyncRequestVector> =
            std::collections::HashMap::new();

        log::trace!(
            "[PERF]     sync_rprims: candidates={} dirty={} skipped={} non_varying={}",
            num_candidates,
            dirty.len(),
            num_skipped,
            num_non_varying
        );
        let mut t_sync_total = std::time::Duration::ZERO;
        let mut t_delegate_lookup = std::time::Duration::ZERO;
        // 5d. Sync each dirty rprim.
        for (id, mut bits) in dirty {
            // Accumulate into per-delegate bucket (P0-3)
            let dl_t0 = std::time::Instant::now();
            let delegate_id = self
                .rprims
                .get(&id)
                .map(|e| e.scene_delegate_id.clone())
                .unwrap_or_else(SdfPath::absolute_root);
            let req = delegate_sync_map.entry(delegate_id).or_default();
            req.ids.push(id.clone());
            req.dirty_bits.push(bits);
            t_delegate_lookup += dl_t0.elapsed();

            let sync_t0 = std::time::Instant::now();
            if let Some(entry) = self.rprims.get_mut(&id) {
                if let Some(ref mut sh) = entry.sync_handle {
                    // Direct sync via HdRprimSync with `_ref`-ordered pre-sync.
                    let _ = Self::pre_sync_typed_rprim(
                        &mut self.tracker,
                        &id,
                        sh,
                        &mut bits,
                        &repr_selector,
                        &repr_token,
                    );
                    sh.sync_dyn(scene_delegate, None, &mut bits, &repr_token);
                } else {
                    // Fallback: delegate dispatch (backends overriding sync_rprim).
                    let delegate = self.render_delegate.read();
                    delegate.sync_rprim(
                        &mut entry.handle,
                        &id,
                        scene_delegate,
                        &mut bits,
                        &repr_token,
                    );
                }
            }
            t_sync_total += sync_t0.elapsed();
            // 5e. Mark clean, preserving Varying bit (C++ behavior).
            let clean_bits = bits & crate::change_tracker::HdRprimDirtyBits::VARYING;
            self.tracker.mark_rprim_clean(&id, clean_bits);
        }

        log::trace!(
            "[PERF]     sync_rprims breakdown: delegate_lookup={:.1}ms sync_total={:.1}ms",
            t_delegate_lookup.as_secs_f64() * 1000.0,
            t_sync_total.as_secs_f64() * 1000.0
        );
        // 5f. Log the per-delegate request map (actual sync() call requires &mut delegate).
        // Callers with &mut delegate should use sync_rprims_with_delegate_mut which calls
        // delegate.sync() + delegate.post_sync_cleanup() after the loop.
        log::trace!(
            "sync_rprims_impl: {} delegate buckets, {} total rprims synced",
            delegate_sync_map.len(),
            delegate_sync_map
                .values()
                .map(|r| r.ids.len())
                .sum::<usize>()
        );

        // 5g. Apply dirty list pruning heuristics, then clear set.
        if num_candidates > MIN_DIRTY_LIST_SIZE {
            let ratio_skipped = num_skipped as f32 / num_candidates as f32;
            let ratio_non_varying = num_non_varying as f32 / num_candidates as f32;
            if ratio_skipped > MIN_RATIO_SKIPPED {
                self.tracker.reset_varying_state();
            } else if ratio_non_varying > MIN_RATIO_NON_VARYING {
                self.dirty_rprim_ids.retain(|id| {
                    let bits = self.tracker.get_rprim_dirty_bits(id);
                    crate::change_tracker::HdRprimDirtyBits::is_varying(bits)
                });
                return;
            }
        }
        self.dirty_rprim_ids.clear();
    }

    /// Sync rprims with a mutable delegate reference — enables calling
    /// delegate.sync() and delegate.post_sync_cleanup() per C++ SyncAll.
    ///
    /// Matches C++ `HdRenderIndex::SyncAll` rprim phase + clean-up phase.
    /// Called from `sync_all_with_delegate_mut` when the caller owns the delegate.
    fn pre_sync_typed_rprim(
        tracker: &mut HdChangeTracker,
        id: &SdfPath,
        sync_handle: &mut HdRprimHandle,
        bits: &mut HdDirtyBits,
        repr_selector: &crate::prim::HdReprSelector,
        repr_token: &Token,
    ) -> bool {
        use crate::change_tracker::HdRprimDirtyBits;

        if (*bits & (HdRprimDirtyBits::INIT_REPR | HdRprimDirtyBits::DIRTY_REPR)) != 0 {
            sync_handle.set_repr_selector_dyn(repr_selector.clone());
            sync_handle.init_repr_dyn(repr_token, bits);
            *bits &= !HdRprimDirtyBits::INIT_REPR;
            tracker.mark_rprim_clean(id, *bits);
        }

        if sync_handle.can_skip_dirty_bit_propagation_and_sync_dyn(*bits) {
            *bits = HdRprimDirtyBits::CLEAN;
            tracker.reset_rprim_varying_state(id);
            return false;
        }

        *bits = sync_handle.propagate_dirty_bits_dyn(*bits);
        if crate::change_tracker::HdRprimDirtyBits::is_dirty(*bits) {
            true
        } else {
            tracker.reset_rprim_varying_state(id);
            false
        }
    }

    pub fn sync_rprims_with_delegate_mut(
        &mut self,
        scene_delegate: &mut dyn crate::prim::HdSceneDelegate,
    ) {
        let total_started = std::time::Instant::now();
        let diag_sync = std::env::var_os("USD_PROFILE_SYNC").is_some();
        let diag = |msg: &str| {
            if diag_sync {
                eprintln!("[render_index::sync_rprims] {msg}");
            }
        };
        // 5a. Collect dirty rprim ids, tracking skip/non-varying counts for heuristics.
        const MIN_DIRTY_LIST_SIZE: usize = 500;
        const MIN_RATIO_SKIPPED: f32 = 0.25;
        const MIN_RATIO_NON_VARYING: f32 = 0.10;

        let candidates: Vec<SdfPath> = self.dirty_rprim_ids.iter().cloned().collect();
        let num_candidates = candidates.len();
        let mut num_skipped: usize = 0;
        let mut num_non_varying: usize = 0;

        let mut dirty: Vec<(SdfPath, HdDirtyBits)> = Vec::with_capacity(candidates.len());
        for id in &candidates {
            let bits = self.tracker.get_rprim_dirty_bits(id);
            if !crate::change_tracker::HdRprimDirtyBits::is_varying(bits) {
                num_non_varying += 1;
            }
            if crate::change_tracker::HdRprimDirtyBits::is_dirty(bits) {
                dirty.push((id.clone(), bits));
            } else {
                num_skipped += 1;
            }
        }

        if dirty.is_empty() {
            self.dirty_rprim_ids.clear();
            return;
        }

        let debug_flo_dirty = flo_debug_enabled();
        if debug_flo_dirty {
            eprintln!(
                "[dirty-trace] stage=render_index_collect emitter=HdRenderIndex candidates={} dirty={} skipped={} non_varying={}",
                num_candidates,
                dirty.len(),
                num_skipped,
                num_non_varying,
            );
            reset_debug_flattened_xform_stats();
            reset_debug_transform_stats();
            reset_debug_xform_schema_stats();
            reset_debug_flattening_xform_get_stats();
        }

        if flo_debug_enabled() {
            use crate::change_tracker::HdRprimDirtyBits;
            let mut transform_only = 0usize;
            let mut with_non_transform = 0usize;
            let mut has_transform = 0usize;
            let mut has_extent = 0usize;
            let mut has_visibility = 0usize;
            let mut has_points = 0usize;
            let mut has_primvar = 0usize;
            let mut has_normals = 0usize;
            let mut has_instancer = 0usize;
            let mut has_instance_index = 0usize;
            for (_, bits) in &dirty {
                // `mark_rprim_dirty()` preserves/sets `VARYING` on animated prims,
                // so classify "transform-only" after masking that bookkeeping bit.
                let scene_bits = *bits & !HdRprimDirtyBits::VARYING;
                if scene_bits == HdRprimDirtyBits::DIRTY_TRANSFORM {
                    transform_only += 1;
                } else {
                    with_non_transform += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_TRANSFORM) != 0 {
                    has_transform += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_EXTENT) != 0 {
                    has_extent += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
                    has_visibility += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_POINTS) != 0 {
                    has_points += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                    has_primvar += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_NORMALS) != 0 {
                    has_normals += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_INSTANCER) != 0 {
                    has_instancer += 1;
                }
                if (*bits & HdRprimDirtyBits::DIRTY_INSTANCE_INDEX) != 0 {
                    has_instance_index += 1;
                }
            }
            eprintln!(
                "[render_index-debug] candidates={} dirty={} transform_only={} non_transform_or_mixed={} has_transform={} has_extent={} has_visibility={} has_points={} has_primvar={} has_normals={} has_instancer={} has_instance_index={}",
                num_candidates,
                dirty.len(),
                transform_only,
                with_non_transform,
                has_transform,
                has_extent,
                has_visibility,
                has_points,
                has_primvar,
                has_normals,
                has_instancer,
                has_instance_index,
            );
        }

        // 5b. Sync dirty instancers first.
        diag("sync_instancers");
        self.sync_instancers();

        // 5c. Determine active repr token.
        let repr_token = self
            .collections_to_sync
            .first()
            .and_then(|c| {
                let tok = c.get_repr_selector().get_token(0);
                if tok.as_str().is_empty() {
                    None
                } else {
                    Some(tok.clone())
                }
            })
            .unwrap_or_else(|| Token::new("refined"));
        let repr_selector = self
            .collections_to_sync
            .first()
            .map(|c| c.get_repr_selector().clone())
            .unwrap_or_else(|| crate::prim::HdReprSelector::with_token(repr_token.clone()));

        // 5d. Pre-sync rprims before building the aggregate delegate request.
        diag("pre_sync_rprims");
        let mut prepared_dirty: Vec<(SdfPath, HdDirtyBits)> = Vec::with_capacity(dirty.len());
        let mut delegate_batch_entries: Vec<(SdfPath, HdDirtyBits)> = Vec::new();
        let mut sync_handle_count = 0usize;
        let tracker = &mut self.tracker;
        for (id, mut bits) in dirty {
            if let Some(entry) = self.rprims.get_mut(&id) {
                if let Some(ref mut sh) = entry.sync_handle {
                    sync_handle_count += 1;
                    if Self::pre_sync_typed_rprim(
                        tracker,
                        &id,
                        sh,
                        &mut bits,
                        &repr_selector,
                        &repr_token,
                    ) {
                        prepared_dirty.push((id, bits));
                    }
                } else {
                    delegate_batch_entries.push((id, bits));
                }
            }
        }

        if !delegate_batch_entries.is_empty() {
            diag("pre_sync_rprims_batch");
            let delegate = self.render_delegate.read();
            let mut handles: Vec<*mut super::HdPrimHandle> =
                Vec::with_capacity(delegate_batch_entries.len());
            for (id, _) in &delegate_batch_entries {
                if let Some(entry) = self.rprims.get_mut(id) {
                    handles.push(&mut entry.handle as *mut _);
                } else {
                    handles.push(std::ptr::null_mut());
                }
            }
            let mut batch: Vec<(&SdfPath, &mut super::HdPrimHandle, &mut HdDirtyBits)> =
                Vec::with_capacity(delegate_batch_entries.len());
            for (i, (id, bits)) in delegate_batch_entries.iter_mut().enumerate() {
                let ptr = handles[i];
                if !ptr.is_null() {
                    #[allow(unsafe_code)]
                    let handle = unsafe { &mut *ptr };
                    batch.push((id, handle, bits));
                }
            }
            delegate.pre_sync_rprims_batch(&mut batch, scene_delegate, &repr_token);
            drop(delegate);
            for (id, bits) in delegate_batch_entries.drain(..) {
                if crate::change_tracker::HdRprimDirtyBits::is_dirty(bits) {
                    prepared_dirty.push((id, bits));
                } else {
                    self.tracker.reset_rprim_varying_state(&id);
                }
            }
        }

        // Build aggregate sync request for delegate.sync() from pre-synced dirty bits.
        let mut aggregate = crate::HdSyncRequestVector::default();
        aggregate.ids.reserve(prepared_dirty.len());
        aggregate.dirty_bits.reserve(prepared_dirty.len());
        for (id, bits) in &prepared_dirty {
            aggregate.ids.push(id.clone());
            aggregate.dirty_bits.push(*bits);
        }

        if aggregate.ids.is_empty() {
            self.dirty_rprim_ids.clear();
            return;
        }

        // Matches OpenUSD SyncAll ordering: delegate sync happens before
        // rprim sync so delegate-backed data is ready when rprims pull it.
        diag("scene_delegate.sync");
        let delegate_sync_started = std::time::Instant::now();
        scene_delegate.sync(&mut aggregate);
        let delegate_sync_ms = delegate_sync_started.elapsed().as_secs_f64() * 1000.0;

        // 5e. Sync dirty rprims using the bits prepared in the dedicated pre-sync pass.
        // Split into two groups: those with sync_handle (custom sync path)
        // and those using render_delegate (batch-parallelizable).
        diag("sync_rprims_typed");
        let mut delegate_batch_ids: Vec<(SdfPath, HdDirtyBits)> = Vec::new();
        for (id, mut bits) in prepared_dirty {
            if let Some(entry) = self.rprims.get_mut(&id) {
                if let Some(ref mut sh) = entry.sync_handle {
                    if diag_sync {
                        eprintln!(
                            "[render_index::sync_rprims] sync_rprims_typed:before_sync_dyn id={} type={}",
                            id, entry.type_id
                        );
                    }
                    sh.sync_dyn(scene_delegate, None, &mut bits, &repr_token);
                    if diag_sync {
                        eprintln!(
                            "[render_index::sync_rprims] sync_rprims_typed:after_sync_dyn id={} type={}",
                            id, entry.type_id
                        );
                    }
                    let clean_bits = bits & crate::change_tracker::HdRprimDirtyBits::VARYING;
                    if diag_sync {
                        eprintln!(
                            "[render_index::sync_rprims] sync_rprims_typed:mark_clean id={} type={}",
                            id, entry.type_id
                        );
                    }
                    self.tracker.mark_rprim_clean(&id, clean_bits);
                } else {
                    delegate_batch_ids.push((id, bits));
                }
            }
        }

        // Batch-parallel sync for delegate-managed rprims.
        let batch_count = delegate_batch_ids.len();
        let batch_sync_started = std::time::Instant::now();
        if !delegate_batch_ids.is_empty() {
            diag("sync_rprims_batch");
            let delegate = self.render_delegate.read();
            // Collect (path, &mut handle, &mut bits) using a temporary Vec
            // to avoid multiple HashMap borrows.
            let mut batch_entries: Vec<(SdfPath, HdDirtyBits)> = delegate_batch_ids;
            {
                // Build the batch slice by iterating batch_entries and looking up handles.
                // We process one at a time to satisfy borrow checker.
                let mut handles: Vec<*mut super::HdPrimHandle> =
                    Vec::with_capacity(batch_entries.len());
                for (id, _) in &batch_entries {
                    if let Some(entry) = self.rprims.get_mut(id) {
                        handles.push(&mut entry.handle as *mut _);
                    } else {
                        handles.push(std::ptr::null_mut());
                    }
                }
                // Build batch refs from raw pointers (safe: each points to a different entry).
                let mut batch: Vec<(&SdfPath, &mut super::HdPrimHandle, &mut HdDirtyBits)> =
                    Vec::with_capacity(batch_entries.len());
                for (i, (id, bits)) in batch_entries.iter_mut().enumerate() {
                    let ptr = handles[i];
                    if !ptr.is_null() {
                        // SAFETY: each pointer is to a unique HashMap entry,
                        // no aliasing possible since ids are unique.
                        #[allow(unsafe_code)]
                        let handle = unsafe { &mut *ptr };
                        batch.push((id, handle, bits));
                    }
                }
                delegate.sync_rprims_batch(&mut batch, scene_delegate, &repr_token);
            }
            drop(delegate);
            for (id, bits) in &batch_entries {
                let clean_bits = bits & crate::change_tracker::HdRprimDirtyBits::VARYING;
                self.tracker.mark_rprim_clean(id, clean_bits);
            }
        }

        // 5e. Post-sync cleanup after rprim sync.
        // Matches C++ renderIndex.cpp:1854-1864.
        diag("post_sync_cleanup");
        scene_delegate.post_sync_cleanup();

        // 5f. Apply dirty list pruning heuristics, then clear.
        if num_candidates > MIN_DIRTY_LIST_SIZE {
            let ratio_skipped = num_skipped as f32 / num_candidates as f32;
            let ratio_non_varying = num_non_varying as f32 / num_candidates as f32;
            if ratio_skipped > MIN_RATIO_SKIPPED {
                self.tracker.reset_varying_state();
            } else if ratio_non_varying > MIN_RATIO_NON_VARYING {
                if debug_flo_dirty {
                    let xform_stats = read_debug_flattened_xform_stats();
                    let stats = read_debug_transform_stats();
                    let xform_schema_stats = read_debug_xform_schema_stats();
                    let flattening_xform_get_stats = read_debug_flattening_xform_get_stats();
                    eprintln!(
                        "[scene_delegate-debug] get_transform_calls={} cached_transform_hits={} matrix_ds_hits={} matrix_ds_misses={} prim_ds_hits={} prim_ds_misses={} input_prim_calls={} input_prim_cache_hits={} input_prim_scene_queries={} input_prim_total_ms={:.2} get_transform_total_ms={:.2}",
                        stats.get_transform_calls,
                        stats.get_transform_cache_hits,
                        stats.get_transform_matrix_ds_hits,
                        stats.get_transform_matrix_ds_misses,
                        stats.get_prim_ds_hits,
                        stats.get_prim_ds_misses,
                        stats.get_input_prim_calls,
                        stats.get_input_prim_cache_hits,
                        stats.get_input_prim_scene_queries,
                        stats.get_input_prim_total_ns as f64 / 1_000_000.0,
                        stats.get_transform_total_ns as f64 / 1_000_000.0,
                    );
                    eprintln!(
                        "[xform_schema-debug] get_from_parent_calls={} get_from_parent_total_ms={:.2} get_matrix_calls={} direct_hits={} fallback_hits={} misses={} get_matrix_total_ms={:.2}",
                        xform_schema_stats.get_from_parent_calls,
                        xform_schema_stats.get_from_parent_total_ns as f64 / 1_000_000.0,
                        xform_schema_stats.get_matrix_calls,
                        xform_schema_stats.get_matrix_direct_hits,
                        xform_schema_stats.get_matrix_fallback_hits,
                        xform_schema_stats.get_matrix_misses,
                        xform_schema_stats.get_matrix_total_ns as f64 / 1_000_000.0,
                    );
                    eprintln!(
                        "[flattening_xform_get-debug] calls={} cache_hits={} computes={} total_ms={:.2} compute_ms={:.2}",
                        flattening_xform_get_stats.calls,
                        flattening_xform_get_stats.cache_hits,
                        flattening_xform_get_stats.computes,
                        flattening_xform_get_stats.total_ns as f64 / 1_000_000.0,
                        flattening_xform_get_stats.compute_ns as f64 / 1_000_000.0,
                    );
                    eprintln!(
                        "[flattened_xform-debug] parent_matrix_calls={} matrix_cache_hits={} matrix_cache_misses={} typed_value_calls={} typed_value_cache_hits={} typed_value_cache_misses={} parent_matrix_total_ms={:.2} typed_value_total_ms={:.2}",
                        xform_stats.parent_matrix_calls,
                        xform_stats.matrix_cache_hits,
                        xform_stats.matrix_cache_misses,
                        xform_stats.typed_value_calls,
                        xform_stats.typed_value_cache_hits,
                        xform_stats.typed_value_cache_misses,
                        xform_stats.parent_matrix_total_ns as f64 / 1_000_000.0,
                        xform_stats.typed_value_total_ns as f64 / 1_000_000.0,
                    );
                }
                self.dirty_rprim_ids.retain(|id| {
                    let bits = self.tracker.get_rprim_dirty_bits(id);
                    crate::change_tracker::HdRprimDirtyBits::is_varying(bits)
                });
                log::debug!(
                    "[render_index] sync_rprims_with_delegate_mut dirty={} sync_handle={} batch={} delegate_sync_ms={:.2} batch_ms={:.2} total_ms={:.2} retained_varying=true",
                    aggregate.ids.len(),
                    sync_handle_count,
                    batch_count,
                    delegate_sync_ms,
                    batch_sync_started.elapsed().as_secs_f64() * 1000.0,
                    total_started.elapsed().as_secs_f64() * 1000.0
                );
                return;
            }
        }
        self.dirty_rprim_ids.clear();
        diag("done");
        if debug_flo_dirty {
            let xform_stats = read_debug_flattened_xform_stats();
            let stats = read_debug_transform_stats();
            let xform_schema_stats = read_debug_xform_schema_stats();
            let flattening_xform_get_stats = read_debug_flattening_xform_get_stats();
            eprintln!(
                "[scene_delegate-debug] get_transform_calls={} cached_transform_hits={} matrix_ds_hits={} matrix_ds_misses={} prim_ds_hits={} prim_ds_misses={} input_prim_calls={} input_prim_cache_hits={} input_prim_scene_queries={} input_prim_total_ms={:.2} get_transform_total_ms={:.2}",
                stats.get_transform_calls,
                stats.get_transform_cache_hits,
                stats.get_transform_matrix_ds_hits,
                stats.get_transform_matrix_ds_misses,
                stats.get_prim_ds_hits,
                stats.get_prim_ds_misses,
                stats.get_input_prim_calls,
                stats.get_input_prim_cache_hits,
                stats.get_input_prim_scene_queries,
                stats.get_input_prim_total_ns as f64 / 1_000_000.0,
                stats.get_transform_total_ns as f64 / 1_000_000.0,
            );
            eprintln!(
                "[xform_schema-debug] get_from_parent_calls={} get_from_parent_total_ms={:.2} get_matrix_calls={} direct_hits={} fallback_hits={} misses={} get_matrix_total_ms={:.2}",
                xform_schema_stats.get_from_parent_calls,
                xform_schema_stats.get_from_parent_total_ns as f64 / 1_000_000.0,
                xform_schema_stats.get_matrix_calls,
                xform_schema_stats.get_matrix_direct_hits,
                xform_schema_stats.get_matrix_fallback_hits,
                xform_schema_stats.get_matrix_misses,
                xform_schema_stats.get_matrix_total_ns as f64 / 1_000_000.0,
            );
            eprintln!(
                "[flattening_xform_get-debug] calls={} cache_hits={} computes={} total_ms={:.2} compute_ms={:.2}",
                flattening_xform_get_stats.calls,
                flattening_xform_get_stats.cache_hits,
                flattening_xform_get_stats.computes,
                flattening_xform_get_stats.total_ns as f64 / 1_000_000.0,
                flattening_xform_get_stats.compute_ns as f64 / 1_000_000.0,
            );
            eprintln!(
                "[flattened_xform-debug] parent_matrix_calls={} matrix_cache_hits={} matrix_cache_misses={} typed_value_calls={} typed_value_cache_hits={} typed_value_cache_misses={} parent_matrix_total_ms={:.2} typed_value_total_ms={:.2}",
                xform_stats.parent_matrix_calls,
                xform_stats.matrix_cache_hits,
                xform_stats.matrix_cache_misses,
                xform_stats.typed_value_calls,
                xform_stats.typed_value_cache_hits,
                xform_stats.typed_value_cache_misses,
                xform_stats.parent_matrix_total_ns as f64 / 1_000_000.0,
                xform_stats.typed_value_total_ns as f64 / 1_000_000.0,
            );
        }
        log::debug!(
            "[render_index] sync_rprims_with_delegate_mut dirty={} sync_handle={} batch={} delegate_sync_ms={:.2} batch_ms={:.2} total_ms={:.2}",
            aggregate.ids.len(),
            sync_handle_count,
            batch_count,
            delegate_sync_ms,
            batch_sync_started.elapsed().as_secs_f64() * 1000.0,
            total_started.elapsed().as_secs_f64() * 1000.0
        );
    }

    /// Full SyncAll pipeline with a mutable scene delegate.
    ///
    /// Like `sync_all_with_delegate` but the delegate is mutable, enabling
    /// `delegate.sync()` and `delegate.post_sync_cleanup()` calls (C++ parity).
    pub fn sync_all_with_delegate_mut(
        &mut self,
        tasks: &mut [HdTaskSharedPtr],
        task_context: &mut HdTaskContext,
        scene_delegate: &mut dyn crate::prim::HdSceneDelegate,
    ) {
        // Phase 1: Render delegate Update
        if self.scene_index_emulation_enabled {
            let mut delegate = self.render_delegate.write();
            delegate.update();
        }

        let had_dirty_sprims = self.has_dirty_sprims();

        // Phases 2-3: Sync bprims and sprims (immutable delegate ok here)
        self.sync_bprims_impl(scene_delegate);
        self.sync_sprims_impl(scene_delegate);

        // Phase 4: Task sync
        self.sync_tasks_with(tasks, task_context, scene_delegate);

        // Phase 5: Rprim sync with delegate.sync() + post_sync_cleanup()
        self.sync_rprims_with_delegate_mut(scene_delegate);
        if had_dirty_sprims {
            scene_delegate.post_sync_cleanup();
        }

        // Phase 6: Clean up
        self.collections_to_sync.clear();
    }

    /// Clear the VARYING bit on an rprim when it has no time-varying data.
    ///
    /// P1-2: VARYING is set whenever any dirty bits are raised on an rprim.
    /// After sync, if the prim has no animated attributes, VARYING should be cleared
    /// so it is not re-evaluated every frame. Call this from the rprim's sync
    /// implementation when it determines the prim is static.
    pub fn clear_rprim_varying(&mut self, id: &SdfPath) {
        self.tracker.reset_rprim_varying_state(id);
    }

    /// Sync dirty instancers.
    ///
    /// Iterates dirty instancers, calls Sync on each, then clears dirty bits.
    /// Matches C++ HdRenderIndex::SyncAll instancer phase.
    fn sync_instancers(&mut self) {
        // Collect dirty instancer IDs under separate borrow
        let dirty_ids: Vec<SdfPath> = self
            .instancers
            .keys()
            .filter(|id| self.tracker.get_instancer_dirty_bits(id) != 0)
            .cloned()
            .collect();

        for id in dirty_ids {
            let mut bits = self.tracker.get_instancer_dirty_bits(&id);
            if bits == 0 {
                continue;
            }
            // Call sync on the instancer (base impl is a no-op; renderers override)
            if let Some(entry) = self.instancers.get_mut(&id) {
                entry.instancer.sync(&NULL_SCENE_DELEGATE, None, &mut bits);
            }
            // Clear dirty bits after sync
            self.tracker.mark_instancer_clean(&id, 0);
        }
    }

    /// Returns true if any sprim currently carries dirty bits in the tracker.
    ///
    /// `_ref` performs an additional `PostSyncCleanup()` pass for scene
    /// delegates backing dirty sprims. This helper preserves that behavior for
    /// the single-delegate Rust pipeline, including the case where no rprims
    /// are dirty in the frame.
    fn has_dirty_sprims(&self) -> bool {
        self.sprims
            .keys()
            .any(|id| self.tracker.get_sprim_dirty_bits(id) != 0)
    }

    /// Clear all prims and tasks from the index.
    ///
    /// Matches C++ HdRenderIndex::Clear. Removes all rprims, sprims, bprims,
    /// instancers, and tasks. Updates change tracker for each removal.
    pub fn clear(&mut self) {
        // Remove all rprims
        let rprim_ids: Vec<_> = self.rprims.keys().cloned().collect();
        for id in rprim_ids {
            self.remove_rprim(&id);
        }

        // Remove all sprims (collect type_id + path before mutating)
        let sprim_ids: Vec<_> = self
            .sprims
            .iter()
            .map(|(id, e)| (id.clone(), e.type_id.clone()))
            .collect();
        for (id, type_id) in sprim_ids {
            self.remove_sprim(&type_id, &id);
        }

        // Remove all bprims
        let bprim_ids: Vec<_> = self
            .bprims
            .iter()
            .map(|(id, e)| (id.clone(), e.type_id.clone()))
            .collect();
        for (id, type_id) in bprim_ids {
            self.remove_bprim(&type_id, &id);
        }

        // Remove all instancers
        let instancer_ids: Vec<_> = self.instancers.keys().cloned().collect();
        for id in instancer_ids {
            self.remove_instancer(&id);
        }

        // Remove all tasks (tracker notified per task)
        let task_ids: Vec<_> = self.tasks.keys().cloned().collect();
        for id in task_ids {
            self.remove_task(&id);
        }

        self.collections_to_sync.clear();
        self.render_tags.clear();
        self.rprim_prim_id_map.clear();
        self.rprim_instancer_ids.clear();
        self.dirty_rprim_ids.clear();
    }

    /// Remove all prims under a given root path belonging to a specific delegate.
    ///
    /// Matches C++ HdRenderIndex::RemoveSubtree.
    pub fn remove_subtree(&mut self, root: &SdfPath, scene_delegate_id: &SdfPath) {
        // Remove matching rprims
        let matching_rprims: Vec<SdfPath> = self
            .rprims
            .iter()
            .filter(|(id, _)| {
                // scene_delegate is not stored per-task; match by id prefix only
                id.has_prefix(root) && id.has_prefix(scene_delegate_id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in matching_rprims {
            self.remove_rprim(&id);
        }

        // Remove matching sprims
        let matching_sprims: Vec<(SdfPath, Token)> = self
            .sprims
            .iter()
            .filter(|(id, entry)| {
                id.has_prefix(root) && &entry.scene_delegate_id == scene_delegate_id
            })
            .map(|(id, entry)| (id.clone(), entry.type_id.clone()))
            .collect();
        for (id, type_id) in matching_sprims {
            self.remove_sprim(&type_id, &id);
        }

        // Remove matching bprims
        let matching_bprims: Vec<(SdfPath, Token)> = self
            .bprims
            .iter()
            .filter(|(id, entry)| {
                id.has_prefix(root) && &entry.scene_delegate_id == scene_delegate_id
            })
            .map(|(id, entry)| (id.clone(), entry.type_id.clone()))
            .collect();
        for (id, type_id) in matching_bprims {
            self.remove_bprim(&type_id, &id);
        }

        // Remove matching instancers
        let matching_instancers: Vec<SdfPath> = self
            .instancers
            .iter()
            .filter(|(id, entry)| {
                id.has_prefix(root) && &entry.scene_delegate_id == scene_delegate_id
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in matching_instancers {
            self.remove_instancer(&id);
        }

        // Remove matching tasks
        let matching_tasks: Vec<SdfPath> = self
            .tasks
            .iter()
            .filter(|(id, _entry)| {
                // TaskEntry doesn't store a scene_delegate_id separately;
                // match tasks whose path is under both root and delegate root.
                id.has_prefix(root) && id.has_prefix(scene_delegate_id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in matching_tasks {
            self.remove_task(&id);
        }
    }

    //--------------------------------------------------------------------------
    // Accessors
    //--------------------------------------------------------------------------

    /// Get the render delegate.
    pub fn get_render_delegate(&self) -> &HdRenderDelegateSharedPtr {
        &self.render_delegate
    }

    /// Get the resource registry from the render delegate.
    pub fn get_resource_registry(&self) -> Arc<dyn super::render_delegate::HdResourceRegistry> {
        let delegate = self.render_delegate.read();
        delegate.get_resource_registry()
    }

    /// Get instance name.
    pub fn get_instance_name(&self) -> &str {
        &self.instance_name
    }

    /// Get number of rprims.
    pub fn get_rprim_count(&self) -> usize {
        self.rprims.len()
    }

    /// Get number of sprims.
    pub fn get_sprim_count(&self) -> usize {
        self.sprims.len()
    }

    /// Get number of bprims.
    pub fn get_bprim_count(&self) -> usize {
        self.bprims.len()
    }

    /// Get all rprim ids (sorted).
    ///
    /// Matches C++ HdRenderIndex::GetRprimIds.
    pub fn get_rprim_ids(&self) -> Vec<SdfPath> {
        let mut ids: Vec<_> = self.rprims.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Get all sprim ids.
    pub fn get_sprim_ids(&self) -> Vec<SdfPath> {
        self.sprims.keys().cloned().collect()
    }

    /// Get sprim ids filtered by type.
    ///
    /// Matches C++ Hd_PrimTypeIndex::GetPrimSubtree (for type-filtered queries).
    pub fn get_sprim_ids_for_type(&self, type_id: &Token) -> Vec<SdfPath> {
        self.sprims
            .iter()
            .filter(|(_, entry)| &entry.type_id == type_id)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get all bprim ids.
    pub fn get_bprim_ids(&self) -> Vec<SdfPath> {
        self.bprims.keys().cloned().collect()
    }

    /// Get bprim ids filtered by type.
    ///
    /// Matches C++ Hd_PrimTypeIndex::GetPrimSubtree (for type-filtered queries).
    pub fn get_bprim_ids_for_type(&self, type_id: &Token) -> Vec<SdfPath> {
        self.bprims
            .iter()
            .filter(|(_, entry)| &entry.type_id == type_id)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get the driver vector.
    ///
    /// Matches C++ HdRenderIndex::GetDrivers.
    pub fn get_drivers(&self) -> &HdDriverVector {
        &self.drivers
    }

    //--------------------------------------------------------------------------
    // Draw Items
    //--------------------------------------------------------------------------

    /// Get draw items for prims matching the collection and render tags.
    ///
    /// Port of C++ HdRenderIndex::GetDrawItems. Filters rprims by:
    /// 1. Collection root paths (prim must be under at least one root)
    /// 2. Collection exclude paths (prim must NOT be under any exclude)
    /// 3. Render tags (prim's tag must be in the provided list, or list empty = all)
    ///
    /// Then collects draw items from the render delegate for matching prims.
    /// Callers (e.g. HdStRenderPass) downcast the returned arcs to the
    /// backend-specific draw item type.
    pub fn get_draw_items(
        &self,
        collection: &HdRprimCollection,
        render_tags: &[Token],
    ) -> Vec<Arc<dyn Any + Send + Sync>> {
        let delegate = self.render_delegate.read();

        // Gather rprim IDs sorted for deterministic output (matches C++ _rprimIds.GetIds())
        let mut rprim_ids: Vec<&SdfPath> = self.rprims.keys().collect();
        rprim_ids.sort();

        let root_paths = collection.get_root_paths();
        let exclude_paths = collection.get_exclude_paths();

        let mut result = Vec::new();

        for prim_id in rprim_ids {
            // Filter 1: prim must be under at least one root path
            let under_root =
                root_paths.is_empty() || root_paths.iter().any(|root| prim_id.has_prefix(root));
            if !under_root {
                continue;
            }

            // Filter 2: prim must NOT be under any exclude path
            let excluded = exclude_paths.iter().any(|excl| prim_id.has_prefix(excl));
            if excluded {
                continue;
            }

            // Filter 3: render tag filter (empty = accept all)
            if !render_tags.is_empty() {
                let prim_tag = self.get_render_tag(prim_id);
                if !render_tags.contains(&prim_tag) {
                    continue;
                }
            }

            // Prim passes all filters, collect draw items from render delegate.
            // Pass the sync_handle's Any ref so Storm can read draw items from the
            // typed prim that was actually synced (rather than the opaque handle).
            let entry = &self.rprims[prim_id];
            let sync_any: Option<&dyn Any> = entry.sync_handle.as_ref().map(|sh| sh.as_any_ref());
            let items = delegate.get_draw_items_for_rprim(
                &entry.handle,
                sync_any,
                prim_id,
                collection,
                render_tags,
            );
            result.extend(items);
        }
        result
    }

    //--------------------------------------------------------------------------
    // Prim Lookup (C++ parity: GetRprim, GetSprim, GetBprim)
    //--------------------------------------------------------------------------

    /// Get the opaque rprim handle by path.
    ///
    /// Matches C++ HdRenderIndex::GetRprim.
    pub fn get_rprim(&self, id: &SdfPath) -> Option<&HdPrimHandle> {
        self.rprims.get(id).map(|entry| &entry.handle)
    }

    /// Get the opaque sprim handle by type and path.
    ///
    /// Matches C++ HdRenderIndex::GetSprim. Verifies the stored type matches.
    pub fn get_sprim(&self, type_id: &Token, id: &SdfPath) -> Option<&HdPrimHandle> {
        self.sprims.get(id).and_then(|entry| {
            if &entry.type_id == type_id {
                Some(&entry.handle)
            } else {
                None
            }
        })
    }

    /// Get the opaque bprim handle by type and path.
    ///
    /// Matches C++ HdRenderIndex::GetBprim. Verifies the stored type matches.
    pub fn get_bprim(&self, type_id: &Token, id: &SdfPath) -> Option<&HdPrimHandle> {
        self.bprims.get(id).and_then(|entry| {
            if &entry.type_id == type_id {
                Some(&entry.handle)
            } else {
                None
            }
        })
    }

    //--------------------------------------------------------------------------
    // Fallback Prims (C++ parity: GetFallbackSprim, GetFallbackBprim)
    //--------------------------------------------------------------------------

    /// Get the fallback sprim for the given type.
    ///
    /// Matches C++ HdRenderIndex::GetFallbackSprim.
    /// Lazily creates via render delegate if not yet cached.
    pub fn get_fallback_sprim(&mut self, type_id: &Token) -> Option<&HdPrimHandle> {
        if !self.fallback_sprims.contains_key(type_id) {
            let handle = {
                let mut delegate = self.render_delegate.write();
                delegate.create_fallback_sprim(type_id)
            };
            if let Some(h) = handle {
                self.fallback_sprims.insert(type_id.clone(), h);
            }
        }
        self.fallback_sprims.get(type_id)
    }

    /// Get the fallback bprim for the given type.
    ///
    /// Matches C++ HdRenderIndex::GetFallbackBprim.
    /// Lazily creates via render delegate if not yet cached.
    pub fn get_fallback_bprim(&mut self, type_id: &Token) -> Option<&HdPrimHandle> {
        if !self.fallback_bprims.contains_key(type_id) {
            let handle = {
                let mut delegate = self.render_delegate.write();
                delegate.create_fallback_bprim(type_id)
            };
            if let Some(h) = handle {
                self.fallback_bprims.insert(type_id.clone(), h);
            }
        }
        self.fallback_bprims.get(type_id)
    }

    //--------------------------------------------------------------------------
    // Prim Type Support (C++ parity: Is*TypeSupported)
    //--------------------------------------------------------------------------

    /// Check if the rprim type is supported by the render delegate.
    ///
    /// Matches C++ HdRenderIndex::IsRprimTypeSupported.
    pub fn is_rprim_type_supported(&self, type_id: &Token) -> bool {
        let delegate = self.render_delegate.read();
        delegate.get_supported_rprim_types().contains(type_id)
    }

    /// Check if the sprim type is supported by the render delegate.
    ///
    /// Matches C++ HdRenderIndex::IsSprimTypeSupported.
    pub fn is_sprim_type_supported(&self, type_id: &Token) -> bool {
        let delegate = self.render_delegate.read();
        delegate.get_supported_sprim_types().contains(type_id)
    }

    /// Check if the bprim type is supported by the render delegate.
    ///
    /// Matches C++ HdRenderIndex::IsBprimTypeSupported.
    pub fn is_bprim_type_supported(&self, type_id: &Token) -> bool {
        let delegate = self.render_delegate.read();
        delegate.get_supported_bprim_types().contains(type_id)
    }

    //--------------------------------------------------------------------------
    // Subtree Queries (C++ parity: GetRprimSubtree)
    //--------------------------------------------------------------------------

    /// Get all rprim paths under the given root.
    ///
    /// Matches C++ HdRenderIndex::GetRprimSubtree.
    pub fn get_rprim_subtree(&self, root: &SdfPath) -> Vec<SdfPath> {
        let mut result: Vec<_> = self
            .rprims
            .keys()
            .filter(|id| id.has_prefix(root))
            .cloned()
            .collect();
        result.sort();
        result
    }

    /// Get all sprim paths of a given type under the given root.
    ///
    /// Matches C++ HdRenderIndex::GetSprimSubtree(typeId, root).
    pub fn get_sprim_subtree(&self, type_id: &Token, root: &SdfPath) -> Vec<SdfPath> {
        let mut result: Vec<_> = self
            .sprims
            .iter()
            .filter(|(id, entry)| &entry.type_id == type_id && id.has_prefix(root))
            .map(|(id, _)| id.clone())
            .collect();
        result.sort();
        result
    }

    /// Get all bprim paths of a given type under the given root.
    ///
    /// Matches C++ HdRenderIndex::GetBprimSubtree(typeId, root).
    pub fn get_bprim_subtree(&self, type_id: &Token, root: &SdfPath) -> Vec<SdfPath> {
        let mut result: Vec<_> = self
            .bprims
            .iter()
            .filter(|(id, entry)| &entry.type_id == type_id && id.has_prefix(root))
            .map(|(id, _)| id.clone())
            .collect();
        result.sort();
        result
    }

    //--------------------------------------------------------------------------
    // Render Tags (C++ parity: GetRenderTag)
    //--------------------------------------------------------------------------

    /// Get the render tag for the given rprim.
    ///
    /// Matches C++ HdRenderIndex::GetRenderTag.
    /// Returns "geometry" (default) if no explicit tag is set.
    pub fn get_render_tag(&self, rprim_id: &SdfPath) -> Token {
        self.render_tags
            .get(rprim_id)
            .cloned()
            .unwrap_or_else(|| Token::new("geometry"))
    }

    /// Mark an rprim dirty and add to the dirty acceleration structure.
    ///
    /// Wraps `HdChangeTracker::MarkRprimDirty` and also records the path
    /// in `dirty_rprim_ids` so `sync_rprims` can skip a full scan.
    pub fn mark_rprim_dirty(&mut self, rprim_id: &SdfPath, dirty_bits: HdDirtyBits) {
        self.tracker.mark_rprim_dirty(rprim_id, dirty_bits);
        self.dirty_rprim_ids.insert(rprim_id.clone());
    }

    /// Get the current dirty bits for an rprim from the change tracker.
    pub fn get_rprim_dirty_bits(&self, rprim_id: &SdfPath) -> HdDirtyBits {
        self.tracker.get_rprim_dirty_bits(rprim_id)
    }

    /// Mark every tracked rprim dirty and seed the dirty acceleration set.
    pub fn mark_all_rprims_dirty(&mut self, dirty_bits: HdDirtyBits) {
        self.tracker.mark_all_rprims_dirty(dirty_bits);
        self.dirty_rprim_ids.extend(self.rprims.keys().cloned());
        self.reset_incremental_sync();
    }

    /// Mark an sprim dirty in the change tracker.
    pub fn mark_sprim_dirty(&mut self, sprim_id: &SdfPath, dirty_bits: HdDirtyBits) {
        self.tracker.mark_sprim_dirty(sprim_id, dirty_bits);
    }

    /// Mark a bprim dirty in the change tracker.
    pub fn mark_bprim_dirty(&mut self, bprim_id: &SdfPath, dirty_bits: HdDirtyBits) {
        self.tracker.mark_bprim_dirty(bprim_id, dirty_bits);
    }

    /// Mark an instancer dirty in the change tracker.
    pub fn mark_instancer_dirty(&mut self, instancer_id: &SdfPath, dirty_bits: HdDirtyBits) {
        self.tracker.mark_instancer_dirty(instancer_id, dirty_bits);
    }

    /// Set the render tag for an rprim.
    pub fn set_render_tag(&mut self, rprim_id: &SdfPath, tag: Token) {
        self.render_tags.insert(rprim_id.clone(), tag);
    }

    /// Update the render tag for an rprim if dirty, then mark clean.
    ///
    /// Matches C++ `HdRenderIndex::UpdateRenderTag(SdfPath const& id, HdDirtyBits bits)`.
    /// Returns the current render tag ("hidden" if rprim not found).
    /// Called during rprim sync to refresh the cached render tag.
    pub fn update_render_tag(&mut self, rprim_id: &SdfPath, dirty_bits: HdDirtyBits) -> Token {
        if !self.rprims.contains_key(rprim_id) {
            return Token::new("hidden");
        }

        if dirty_bits & crate::change_tracker::HdRprimDirtyBits::DIRTY_RENDER_TAG != 0 {
            // C++: info->rprim->UpdateRenderTag(sceneDelegate, renderParam)
            // then marks DirtyRenderTag clean. The rprim queries the scene
            // delegate internally and caches the tag.
            // For now we mark the bit clean; the actual tag query will be
            // handled once rprim.Sync() is wired up.
            self.tracker.mark_rprim_clean(
                rprim_id,
                dirty_bits & !crate::change_tracker::HdRprimDirtyBits::DIRTY_RENDER_TAG,
            );
        }

        // Return cached render tag (defaults to "geometry")
        self.get_render_tag(rprim_id)
    }

    //--------------------------------------------------------------------------
    // Scene Delegate Lookup (C++ parity: GetSceneDelegateForRprim)
    //--------------------------------------------------------------------------

    /// Get the scene delegate id for the given rprim.
    ///
    /// Matches C++ HdRenderIndex::GetSceneDelegateForRprim.
    /// Returns the SdfPath of the scene delegate that owns this rprim.
    pub fn get_scene_delegate_for_rprim(&self, id: &SdfPath) -> Option<&SdfPath> {
        self.rprims.get(id).map(|entry| &entry.scene_delegate_id)
    }

    //--------------------------------------------------------------------------
    // Rprim ID Allocation (C++ parity: _AllocatePrimId)
    //--------------------------------------------------------------------------

    /// Allocate a unique monotonic rprim ID for picking/selection.
    ///
    /// Matches C++ HdRenderIndex::_AllocatePrimId (24-bit IDs for color picking).
    /// Returns an incrementing i32; wraps are handled by _compact_prim_ids.
    pub fn allocate_rprim_id(&mut self) -> i32 {
        let id = self.rprim_prim_id_map.len() as i32;
        id
    }

    /// Internal: allocate a prim ID and register it in the map.
    ///
    /// Matches C++ `_AllocatePrimId(HdRprim*)` with 24-bit wrap-around.
    fn _allocate_prim_id(&mut self, prim_id: &SdfPath) {
        const MAX_ID: usize = (1 << 24) - 1;
        if self.rprim_prim_id_map.len() > MAX_ID {
            // Wrap-around: compact and reassign
            self._compact_prim_ids();
        }
        self.rprim_prim_id_map.push(prim_id.clone());
    }

    /// Given a prim id (integer), returns the path of the rprim or empty path.
    ///
    /// Matches C++ `HdRenderIndex::GetRprimPathFromPrimId` (renderIndex.cpp:1911-1918).
    /// Used for picking/selection: color buffer encodes prim ID, this resolves to path.
    pub fn get_rprim_path_from_prim_id(&self, prim_id: i32) -> SdfPath {
        if prim_id < 0 || (prim_id as usize) >= self.rprim_prim_id_map.len() {
            return SdfPath::empty();
        }
        self.rprim_prim_id_map[prim_id as usize].clone()
    }

    /// Returns the current prim id assigned to the given rprim path.
    ///
    /// This is the canonical source for ID-render picking and stays valid even
    /// when the global prim-id map contains tombstones from removed rprims.
    pub fn get_prim_id_for_rprim_path(&self, rprim_path: &SdfPath) -> Option<i32> {
        self.rprims.get(rprim_path).map(|entry| entry.prim_id)
    }

    /// Compact prim IDs by reassigning sequentially.
    ///
    /// Matches C++ `HdRenderIndex::_CompactPrimIds` (renderIndex.cpp:1882-1892).
    /// Called when 24-bit ID space wraps around. Reassigns IDs contiguously,
    /// writes new IDs back into RprimEntry (P0-1), and marks all rprims
    /// DirtyPrimID so shaders pick up the new IDs.
    fn _compact_prim_ids(&mut self) {
        self.rprim_prim_id_map.clear();
        self.rprim_prim_id_map.reserve(self.rprims.len());
        // Collect paths first to satisfy borrow checker
        let paths: Vec<SdfPath> = self.rprims.keys().cloned().collect();
        for path in paths {
            let new_id = self.rprim_prim_id_map.len() as i32;
            self.rprim_prim_id_map.push(path.clone());
            // P0-1: write back the new compact ID into the entry
            if let Some(entry) = self.rprims.get_mut(&path) {
                entry.prim_id = new_id;
            }
            self.tracker.mark_rprim_dirty(
                &path,
                crate::change_tracker::HdRprimDirtyBits::DIRTY_PRIM_ID,
            );
        }
    }

    //--------------------------------------------------------------------------
    // Scene Delegate + Instancer Lookup (C++ parity)
    //--------------------------------------------------------------------------

    /// Query scene delegate and instancer ids for the rprim at the given path.
    ///
    /// Matches C++ `HdRenderIndex::GetSceneDelegateAndInstancerIds`.
    /// Returns (scene_delegate_id, instancer_id) or None if rprim not found.
    #[deprecated(note = "Query terminal scene index for instancer info instead")]
    pub fn get_scene_delegate_and_instancer_ids(&self, id: &SdfPath) -> Option<(SdfPath, SdfPath)> {
        let entry = self.rprims.get(id)?;
        let delegate_id = entry.scene_delegate_id.clone();
        let instancer_id = self
            .rprim_instancer_ids
            .get(id)
            .cloned()
            .unwrap_or_else(SdfPath::empty);
        Some((delegate_id, instancer_id))
    }

    /// Get the instancer path associated with an rprim, or empty path if none.
    ///
    /// Matches C++ `_GetInstancerForPrim`. Used by pick resolution to populate
    /// `hit_instancer_path` in IntersectionResult.
    pub fn get_instancer_id_for_rprim(&self, rprim_id: &SdfPath) -> SdfPath {
        self.rprim_instancer_ids
            .get(rprim_id)
            .cloned()
            .unwrap_or_else(SdfPath::empty)
    }

    /// Set the instancer associated with an rprim.
    ///
    /// Called when an rprim is assigned to an instancer during sync.
    pub fn set_rprim_instancer_id(&mut self, rprim_id: &SdfPath, instancer_id: SdfPath) {
        if instancer_id.is_empty() {
            self.rprim_instancer_ids.remove(rprim_id);
        } else {
            self.rprim_instancer_ids
                .insert(rprim_id.clone(), instancer_id);
        }
    }

    //--------------------------------------------------------------------------
    // Notice Batching (C++ parity: SceneIndexEmulationNoticeBatch*)
    //--------------------------------------------------------------------------

    /// Begin batching scene index emulation notices.
    ///
    /// Matches C++ `HdRenderIndex::SceneIndexEmulationNoticeBatchBegin`.
    /// Supports nested calls via depth tracking.
    pub fn scene_index_emulation_notice_batch_begin(&mut self) {
        self.emulation_batching_ctx.begin_batching();
    }

    /// End batching scene index emulation notices.
    ///
    /// Matches C++ `HdRenderIndex::SceneIndexEmulationNoticeBatchEnd`.
    /// Flushes accumulated notices when depth returns to 0.
    pub fn scene_index_emulation_notice_batch_end(&mut self) {
        self.emulation_batching_ctx.end_batching();
    }

    /// Begin batching merging scene index notices.
    ///
    /// Matches C++ `HdRenderIndex::MergingSceneIndexNoticeBatchBegin`.
    pub fn merging_scene_index_notice_batch_begin(&mut self) {
        self.merging_batching_ctx.begin_batching();
    }

    /// End batching merging scene index notices.
    ///
    /// Matches C++ `HdRenderIndex::MergingSceneIndexNoticeBatchEnd`.
    pub fn merging_scene_index_notice_batch_end(&mut self) {
        self.merging_batching_ctx.end_batching();
    }

    /// Check if emulation batching is active.
    pub fn is_emulation_batching(&self) -> bool {
        self.emulation_batching_ctx.is_batching()
    }

    /// Check if merging batching is active.
    pub fn is_merging_batching(&self) -> bool {
        self.merging_batching_ctx.is_batching()
    }

    //--------------------------------------------------------------------------
    // Prim Type Init / Fallback Prims (C++ parity)
    //--------------------------------------------------------------------------

    /// Initialize prim type indices from supported types.
    ///
    /// Matches C++ `HdRenderIndex::_InitPrimTypes` (renderIndex.cpp:2167-2171).
    /// In C++, this sets up Hd_PrimTypeIndex for sprims/bprims. Here we just
    /// validate the delegate's supported types are registered.
    fn _init_prim_types(&self) {
        // In the full C++ implementation, this initializes Hd_PrimTypeIndex
        // for sprims and bprims. Our HashMap-based approach doesn't need
        // explicit type registration.
        let _rd = self.render_delegate.read();
    }

    /// Create fallback prims for all supported sprim/bprim types.
    ///
    /// Matches C++ `HdRenderIndex::_CreateFallbackPrims` (renderIndex.cpp:969-977).
    /// Fallback prims provide default state when a referenced prim doesn't exist.
    fn _create_fallback_prims(&mut self) {
        let delegate = self.render_delegate.read();
        let sprim_types: Vec<Token> = delegate.get_supported_sprim_types().clone();
        let bprim_types: Vec<Token> = delegate.get_supported_bprim_types().clone();
        drop(delegate);

        for type_id in &sprim_types {
            if !self.fallback_sprims.contains_key(type_id) {
                let handle = {
                    let mut d = self.render_delegate.write();
                    d.create_fallback_sprim(type_id)
                };
                if let Some(h) = handle {
                    self.fallback_sprims.insert(type_id.clone(), h);
                }
            }
        }

        for type_id in &bprim_types {
            if !self.fallback_bprims.contains_key(type_id) {
                let handle = {
                    let mut d = self.render_delegate.write();
                    d.create_fallback_bprim(type_id)
                };
                if let Some(h) = handle {
                    self.fallback_bprims.insert(type_id.clone(), h);
                }
            }
        }
    }

    /// Destroy all fallback prims.
    ///
    /// Matches C++ `HdRenderIndex::_DestroyFallbackPrims` (renderIndex.cpp:980-984).
    /// Called during destruction.
    fn _destroy_fallback_prims(&mut self) {
        self.fallback_sprims.clear();
        self.fallback_bprims.clear();
    }

    /// Register well-known reprs at construction time.
    ///
    /// Ports C++ `HdRenderIndex::_ConfigureReprs` (renderIndex.cpp:988-1097).
    /// Registers all pre-defined repr descriptors on HdMesh, HdBasisCurves,
    /// and HdPoints. Called once at construction via `std::sync::Once`.
    fn _configure_reprs() {
        use crate::enums::{
            HdBasisCurvesGeomStyle, HdCullStyle, HdMeshGeomStyle, HdPointsGeomStyle,
        };
        use crate::prim::basis_curves::HdBasisCurves;
        use crate::prim::mesh::HdMesh;
        use crate::prim::points::HdPoints;
        use crate::prim::{HdBasisCurvesReprDesc, HdMeshReprDesc, HdPointsReprDesc};
        use crate::tokens;

        // Shading terminal tokens.
        let surface_shader = Token::new("surfaceShader");
        let point_color = Token::new("pointColor");

        // Empty secondary descriptor (used when only one draw item per repr).
        let empty_mesh = HdMeshReprDesc::default(); // geom_style = Invalid

        // ----- HdMesh reprs (matches renderIndex.cpp:991-1054) -----

        // hull: flat-shaded coarse hull, no blend wireframe
        HdMesh::configure_repr(
            &tokens::REPR_HULL,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::Hull,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: true,
                blend_wireframe_color: false,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // smoothHull: smooth-shaded coarse hull
        HdMesh::configure_repr(
            &tokens::REPR_SMOOTH_HULL,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::Hull,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: false,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // wire: hull edges only, blend wireframe
        HdMesh::configure_repr(
            &tokens::REPR_WIRE,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::HullEdgeOnly,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: true,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // wireOnSurf: hull edges on hull surface
        HdMesh::configure_repr(
            &tokens::REPR_WIRE_ON_SURF,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::HullEdgeOnSurf,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: true,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // solidWireOnSurf: hull edges on hull surface, opaque edges
        HdMesh::configure_repr(
            &tokens::REPR_SOLID_WIRE_ON_SURF,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::HullEdgeOnSurf,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: true,
                force_opaque_edges: true,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // refined: subdivision surface
        HdMesh::configure_repr(
            &tokens::REPR_REFINED,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::Surf,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: false,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // refinedWire: subdivision edges only
        HdMesh::configure_repr(
            &tokens::REPR_REFINED_WIRE,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::EdgeOnly,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: true,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // refinedWireOnSurf: subdivision edges on subdivision surface
        HdMesh::configure_repr(
            &tokens::REPR_REFINED_WIRE_ON_SURF,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::EdgeOnSurf,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: true,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // refinedSolidWireOnSurf: subdivision edges on surface, opaque edges
        HdMesh::configure_repr(
            &tokens::REPR_REFINED_SOLID_WIRE_ON_SURF,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::EdgeOnSurf,
                cull_style: HdCullStyle::DontCare,
                shading_terminal: surface_shader.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: true,
                force_opaque_edges: true,
                ..Default::default()
            },
            empty_mesh.clone(),
        );
        // points: vertices as points, no culling
        HdMesh::configure_repr(
            &tokens::REPR_POINTS,
            HdMeshReprDesc {
                geom_style: HdMeshGeomStyle::Points,
                cull_style: HdCullStyle::Nothing,
                shading_terminal: point_color.clone(),
                flat_shading_enabled: false,
                blend_wireframe_color: false,
                force_opaque_edges: false,
                ..Default::default()
            },
            empty_mesh,
        );

        // ----- HdBasisCurves reprs (matches renderIndex.cpp:1056-1075) -----

        HdBasisCurves::configure_repr(
            &tokens::REPR_HULL,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Patch,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_SMOOTH_HULL,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Patch,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_WIRE,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Wire,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_WIRE_ON_SURF,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Patch,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_SOLID_WIRE_ON_SURF,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Patch,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_REFINED,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Patch,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_REFINED_WIRE,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Wire,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_REFINED_WIRE_ON_SURF,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Patch,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_REFINED_SOLID_WIRE_ON_SURF,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Patch,
                ..Default::default()
            },
        );
        HdBasisCurves::configure_repr(
            &tokens::REPR_POINTS,
            HdBasisCurvesReprDesc {
                geom_style: HdBasisCurvesGeomStyle::Points,
                ..Default::default()
            },
        );

        // ----- HdPoints reprs (matches renderIndex.cpp:1077-1096) -----

        HdPoints::configure_repr(
            &tokens::REPR_HULL,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_SMOOTH_HULL,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_WIRE,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_WIRE_ON_SURF,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_SOLID_WIRE_ON_SURF,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_REFINED,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_REFINED_WIRE,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_REFINED_WIRE_ON_SURF,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_REFINED_SOLID_WIRE_ON_SURF,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
        HdPoints::configure_repr(
            &tokens::REPR_POINTS,
            HdPointsReprDesc {
                geom_style: HdPointsGeomStyle::Points,
            },
        );
    }

    //--------------------------------------------------------------------------
    // Commit Resources (public API for engine to call after sync)
    //--------------------------------------------------------------------------

    /// Commit resources via the render delegate.
    ///
    /// Called by HdEngine::Execute() AFTER SyncAll().
    /// Matches C++ HdRenderIndex::CommitResources pattern (delegates to
    /// HdRenderDelegate::CommitResources).
    pub fn commit_resources(&mut self) {
        let mut delegate = self.render_delegate.write();
        delegate.commit_resources(&mut self.tracker);
    }
}

impl Drop for HdRenderIndex {
    fn drop(&mut self) {
        self._destroy_fallback_prims();
    }
}

// Implement the HdRenderIndex trait from task.rs for trait object usage
impl super::task::HdRenderIndex for HdRenderIndex {
    fn get_task(&self, id: &SdfPath) -> Option<&HdTaskSharedPtr> {
        self.tasks.get(id).map(|entry| &entry.task)
    }

    fn has_task(&self, id: &SdfPath) -> bool {
        self.tasks.contains_key(id)
    }

    fn get_rprim(&self, id: &SdfPath) -> Option<&HdPrimHandle> {
        self.rprims.get(id).map(|entry| &entry.handle)
    }

    fn get_rprim_ids(&self) -> Vec<SdfPath> {
        crate::render::render_index::HdRenderIndex::get_rprim_ids(self)
    }

    fn get_prim_id_for_rprim_path(&self, rprim_path: &SdfPath) -> Option<i32> {
        crate::render::render_index::HdRenderIndex::get_prim_id_for_rprim_path(self, rprim_path)
    }

    fn get_sprim(&self, type_id: &Token, id: &SdfPath) -> Option<&HdPrimHandle> {
        self.sprims.get(id).and_then(|entry| {
            if &entry.type_id == type_id {
                Some(&entry.handle)
            } else {
                None
            }
        })
    }

    fn get_bprim(&self, type_id: &Token, id: &SdfPath) -> Option<&HdPrimHandle> {
        self.bprims.get(id).and_then(|entry| {
            if &entry.type_id == type_id {
                Some(&entry.handle)
            } else {
                None
            }
        })
    }

    fn get_render_delegate(&self) -> &HdRenderDelegateSharedPtr {
        &self.render_delegate
    }

    fn get_change_tracker(&self) -> &crate::change_tracker::HdChangeTracker {
        &self.tracker
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change_tracker::HdRprimDirtyBits;
    use crate::render::render_delegate::*;

    // Mock resource registry for testing
    struct MockResourceRegistry;

    impl HdResourceRegistry for MockResourceRegistry {}

    // Mock render delegate for testing
    struct MockRenderDelegate {
        resource_registry: Arc<MockResourceRegistry>,
        rprim_count: usize,
        sprim_count: usize,
        bprim_count: usize,
    }

    impl MockRenderDelegate {
        fn new() -> Self {
            Self {
                resource_registry: Arc::new(MockResourceRegistry),
                rprim_count: 0,
                sprim_count: 0,
                bprim_count: 0,
            }
        }
    }

    impl HdRenderDelegate for MockRenderDelegate {
        fn get_supported_rprim_types(&self) -> &TfTokenVector {
            static TYPES: Vec<Token> = Vec::new();
            &TYPES
        }

        fn get_supported_sprim_types(&self) -> &TfTokenVector {
            static TYPES: Vec<Token> = Vec::new();
            &TYPES
        }

        fn get_supported_bprim_types(&self) -> &TfTokenVector {
            static TYPES: Vec<Token> = Vec::new();
            &TYPES
        }

        fn create_rprim(&mut self, _type_id: &Token, _id: SdfPath) -> Option<HdPrimHandle> {
            self.rprim_count += 1;
            Some(Box::new(self.rprim_count))
        }

        fn create_sprim(&mut self, _type_id: &Token, _id: SdfPath) -> Option<HdPrimHandle> {
            self.sprim_count += 1;
            Some(Box::new(self.sprim_count))
        }

        fn create_bprim(&mut self, _type_id: &Token, _id: SdfPath) -> Option<HdPrimHandle> {
            self.bprim_count += 1;
            Some(Box::new(self.bprim_count))
        }

        fn create_instancer(
            &mut self,
            _delegate: &dyn HdSceneDelegate,
            _id: SdfPath,
        ) -> Option<Box<dyn crate::render::render_delegate::HdInstancer>> {
            None
        }

        fn destroy_instancer(
            &mut self,
            _instancer: Box<dyn crate::render::render_delegate::HdInstancer>,
        ) {
        }

        fn create_fallback_sprim(&mut self, _type_id: &Token) -> Option<HdPrimHandle> {
            None
        }

        fn create_fallback_bprim(&mut self, _type_id: &Token) -> Option<HdPrimHandle> {
            None
        }

        fn create_render_pass(
            &mut self,
            _index: &HdRenderIndex,
            _collection: &HdRprimCollection,
        ) -> Option<crate::render::render_delegate::HdRenderPassSharedPtr> {
            None
        }

        fn commit_resources(&mut self, _tracker: &mut crate::change_tracker::HdChangeTracker) {
            // No-op for mock
        }

        fn get_resource_registry(&self) -> Arc<dyn HdResourceRegistry> {
            self.resource_registry.clone()
        }
    }

    fn make_index() -> HdRenderIndex {
        let delegate = Arc::new(RwLock::new(MockRenderDelegate::new()));
        HdRenderIndex::new(delegate, Vec::new(), Some("test".to_string()), None).unwrap()
    }

    #[test]
    fn test_render_index_creation() {
        let index = make_index();
        assert_eq!(index.get_instance_name(), "test");
        assert_eq!(index.get_rprim_count(), 0);
    }

    #[test]
    fn test_resource_registry_access() {
        let index = make_index();
        let _registry = index.get_resource_registry();
    }

    #[test]
    fn test_change_tracker_integration() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();
        let prim_id = SdfPath::from_string("/Delegate/Mesh").unwrap();

        // Insert rprim - should register with tracker
        assert!(index.insert_rprim(&mesh, &delegate_id, &prim_id));

        // Tracker should have dirty bits for this rprim
        let bits = index.tracker.get_rprim_dirty_bits(&prim_id);
        assert_ne!(bits, 0, "Newly inserted rprim should be dirty");

        // Remove rprim - should unregister from tracker
        assert!(index.remove_rprim(&prim_id));
        let bits = index.tracker.get_rprim_dirty_bits(&prim_id);
        assert_eq!(bits, 0, "Removed rprim should have no dirty bits");
    }

    #[test]
    fn test_mark_rprim_dirty_seeds_dirty_acceleration_set() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();
        let prim_id = SdfPath::from_string("/Delegate/Mesh").unwrap();

        assert!(index.insert_rprim(&mesh, &delegate_id, &prim_id));

        index.dirty_rprim_ids.clear();
        index
            .tracker
            .mark_rprim_clean(&prim_id, HdRprimDirtyBits::CLEAN);

        index.mark_rprim_dirty(&prim_id, HdRprimDirtyBits::DIRTY_TRANSFORM);

        assert!(
            index.dirty_rprim_ids.contains(&prim_id),
            "mark_rprim_dirty must seed dirty_rprim_ids so incremental sync can visit the rprim"
        );
        assert_ne!(
            index.tracker.get_rprim_dirty_bits(&prim_id) & HdRprimDirtyBits::DIRTY_TRANSFORM,
            0,
            "mark_rprim_dirty must still update the change tracker"
        );
    }

    #[test]
    fn test_sprim_tracker_integration() {
        let mut index = make_index();
        let camera = Token::new("camera");
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();
        let prim_id = SdfPath::from_string("/Delegate/Camera").unwrap();

        assert!(index.insert_sprim(&camera, &delegate_id, &prim_id));
        let bits = index.tracker.get_sprim_dirty_bits(&prim_id);
        assert_ne!(bits, 0);

        assert!(index.remove_sprim(&camera, &prim_id));
        let bits = index.tracker.get_sprim_dirty_bits(&prim_id);
        assert_eq!(bits, 0);
    }

    #[test]
    fn test_bprim_tracker_integration() {
        let mut index = make_index();
        let rb = Token::new("renderBuffer");
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();
        let prim_id = SdfPath::from_string("/Delegate/RB").unwrap();

        assert!(index.insert_bprim(&rb, &delegate_id, &prim_id));
        let bits = index.tracker.get_bprim_dirty_bits(&prim_id);
        assert_ne!(bits, 0);

        assert!(index.remove_bprim(&rb, &prim_id));
        let bits = index.tracker.get_bprim_dirty_bits(&prim_id);
        assert_eq!(bits, 0);
    }

    #[test]
    fn test_instancer_operations() {
        let mut index = make_index();
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();
        let inst_id = SdfPath::from_string("/Delegate/Instancer").unwrap();

        // Initially no instancers
        assert_eq!(index.get_instancer_count(), 0);
        assert!(!index.has_instancer(&inst_id));

        // Insert instancer
        assert!(index.insert_instancer(&delegate_id, &inst_id));
        assert_eq!(index.get_instancer_count(), 1);
        assert!(index.has_instancer(&inst_id));

        // Get instancer
        let inst = index.get_instancer(&inst_id);
        assert!(inst.is_some());
        assert_eq!(inst.unwrap().get_id(), &inst_id);

        // Tracker should have dirty bits
        let bits = index.tracker.get_instancer_dirty_bits(&inst_id);
        assert_ne!(bits, 0);

        // Can't insert duplicate
        assert!(!index.insert_instancer(&delegate_id, &inst_id));

        // Remove instancer
        assert!(index.remove_instancer(&inst_id));
        assert!(!index.has_instancer(&inst_id));
        assert_eq!(index.get_instancer_count(), 0);
    }

    #[test]
    fn test_nested_instancer() {
        let mut index = make_index();
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();
        let parent_id = SdfPath::from_string("/Delegate/ParentInst").unwrap();
        let child_id = SdfPath::from_string("/Delegate/ChildInst").unwrap();

        // Insert parent first
        assert!(index.insert_instancer(&delegate_id, &parent_id));

        // Insert child with parent dependency
        assert!(index.insert_instancer_with_parent(&delegate_id, &child_id, &parent_id));

        assert_eq!(index.get_instancer_count(), 2);
        let child = index.get_instancer(&child_id).unwrap();
        assert!(child.is_nested());
        assert_eq!(child.get_parent_id(), &parent_id);

        // Marking parent dirty should propagate to child via tracker deps
        index.tracker.mark_instancer_dirty(
            &parent_id,
            crate::change_tracker::HdRprimDirtyBits::DIRTY_TRANSFORM,
        );

        // Clean up
        assert!(index.remove_instancer(&child_id));
        assert!(index.remove_instancer(&parent_id));
    }

    #[test]
    fn test_sync_all_cleans_dirty_prims() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let camera = Token::new("camera");
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();

        // Insert various prims
        let rprim_id = SdfPath::from_string("/Delegate/Mesh").unwrap();
        let sprim_id = SdfPath::from_string("/Delegate/Camera").unwrap();
        let bprim_id = SdfPath::from_string("/Delegate/RB").unwrap();
        let inst_id = SdfPath::from_string("/Delegate/Instancer").unwrap();

        index.insert_rprim(&mesh, &delegate_id, &rprim_id);
        index.insert_sprim(&camera, &delegate_id, &sprim_id);
        index.insert_bprim(&Token::new("renderBuffer"), &delegate_id, &bprim_id);
        index.insert_instancer(&delegate_id, &inst_id);

        // All should be dirty
        assert_ne!(index.tracker.get_rprim_dirty_bits(&rprim_id), 0);
        assert_ne!(index.tracker.get_sprim_dirty_bits(&sprim_id), 0);
        assert_ne!(index.tracker.get_bprim_dirty_bits(&bprim_id), 0);
        assert_ne!(index.tracker.get_instancer_dirty_bits(&inst_id), 0);

        // Run sync
        let mut task_context = HdTaskContext::new();
        index.sync_all(&mut [], &mut task_context);

        // All should be clean now (Varying bit may persist per C++ behavior)
        assert_eq!(index.tracker.get_sprim_dirty_bits(&sprim_id), 0);
        assert_eq!(index.tracker.get_bprim_dirty_bits(&bprim_id), 0);
        // Instancer and rprim mark_clean preserves Varying bit (C++ behavior)
        let varying = crate::change_tracker::HdRprimDirtyBits::VARYING;
        let inst_bits = index.tracker.get_instancer_dirty_bits(&inst_id);
        assert!(
            inst_bits == 0 || inst_bits == varying,
            "Instancer should be clean (possibly Varying): {}",
            inst_bits
        );
    }

    #[test]
    fn test_clear_includes_tasks_and_instancers() {
        let mut index = make_index();
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();

        // Insert instancer
        let inst_id = SdfPath::from_string("/Delegate/Inst").unwrap();
        index.insert_instancer(&delegate_id, &inst_id);

        // Insert rprim
        let mesh_id = SdfPath::from_string("/Delegate/Mesh").unwrap();
        index.insert_rprim(&Token::new("mesh"), &delegate_id, &mesh_id);

        assert_eq!(index.get_instancer_count(), 1);
        assert_eq!(index.get_rprim_count(), 1);

        // Clear should remove everything
        index.clear();
        assert_eq!(index.get_instancer_count(), 0);
        assert_eq!(index.get_rprim_count(), 0);
        assert_eq!(index.get_task_count(), 0);
    }

    #[test]
    fn test_remove_subtree() {
        let mut index = make_index();
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();

        // Insert multiple prims under different roots
        let mesh1 = SdfPath::from_string("/Delegate/World/Mesh1").unwrap();
        let mesh2 = SdfPath::from_string("/Delegate/World/Mesh2").unwrap();
        let mesh3 = SdfPath::from_string("/Delegate/Other/Mesh3").unwrap();

        index.insert_rprim(&Token::new("mesh"), &delegate_id, &mesh1);
        index.insert_rprim(&Token::new("mesh"), &delegate_id, &mesh2);
        index.insert_rprim(&Token::new("mesh"), &delegate_id, &mesh3);

        assert_eq!(index.get_rprim_count(), 3);

        // Remove subtree under /Delegate/World
        let world = SdfPath::from_string("/Delegate/World").unwrap();
        index.remove_subtree(&world, &delegate_id);

        assert_eq!(index.get_rprim_count(), 1);
        assert!(index.has_rprim(&mesh3));
        assert!(!index.has_rprim(&mesh1));
        assert!(!index.has_rprim(&mesh2));
    }

    #[test]
    fn test_enqueue_collection() {
        let mut index = make_index();
        let collection = HdRprimCollection::new(Token::new("geometry"));

        index.enqueue_collection_to_sync(collection);
        assert_eq!(index.collections_to_sync.len(), 1);

        // SyncAll clears collections
        let mut ctx = HdTaskContext::new();
        index.sync_all(&mut [], &mut ctx);
        assert!(index.collections_to_sync.is_empty());
    }

    // Mock task for testing
    use crate::prim::HdSceneDelegate;
    use crate::render::task::{HdRenderIndex as HdRenderIndexTrait, HdTask};
    use parking_lot::RwLock;

    struct MockTask {
        id: SdfPath,
        converged: bool,
    }

    impl HdTask for MockTask {
        fn id(&self) -> &SdfPath {
            &self.id
        }

        fn is_converged(&self) -> bool {
            self.converged
        }

        fn sync(
            &mut self,
            _delegate: &dyn HdSceneDelegate,
            _ctx: &mut HdTaskContext,
            _dirty_bits: &mut u32,
        ) {
        }

        fn prepare(&mut self, _ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {}

        fn execute(&mut self, _ctx: &mut HdTaskContext) {}

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_task_operations() {
        let mut index = make_index();

        assert_eq!(index.get_task_count(), 0);

        let task_id = SdfPath::from_string("/Tasks/RenderTask").unwrap();
        let task: HdTaskSharedPtr = Arc::new(RwLock::new(MockTask {
            id: task_id.clone(),
            converged: true,
        }));

        assert!(index.insert_task(None, &task_id, task));
        assert_eq!(index.get_task_count(), 1);
        assert!(index.has_task(&task_id));

        // Tracker should have task dirty bits
        let bits = index.tracker.get_task_dirty_bits(&task_id);
        assert_ne!(bits, 0, "Newly inserted task should be dirty");

        // Get the task
        let retrieved = index.get_task(&task_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().read().id(), &task_id);

        // Can't insert duplicate
        let task2: HdTaskSharedPtr = Arc::new(RwLock::new(MockTask {
            id: task_id.clone(),
            converged: false,
        }));
        assert!(!index.insert_task(None, &task_id, task2));

        // Remove task
        assert!(index.remove_task(&task_id));
        assert!(!index.has_task(&task_id));
        assert_eq!(index.get_task_count(), 0);
        assert_eq!(index.tracker.get_task_dirty_bits(&task_id), 0);
    }

    #[test]
    fn test_clear_includes_tasks() {
        let mut index = make_index();

        let task_id = SdfPath::from_string("/Tasks/Task1").unwrap();
        let delegate_id = SdfPath::from_string("/Delegate").unwrap();
        let task: HdTaskSharedPtr = Arc::new(RwLock::new(MockTask {
            id: task_id.clone(),
            converged: true,
        }));
        let _ = delegate_id; // no longer needed — insert_task takes Option<Arc<delegate>>
        index.insert_task(None, &task_id, task);
        assert_eq!(index.get_task_count(), 1);

        index.clear();
        assert_eq!(index.get_task_count(), 0);
    }

    #[test]
    fn test_get_rprim() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let delegate_id = SdfPath::from_string("/D").unwrap();
        let prim_id = SdfPath::from_string("/D/Mesh").unwrap();

        assert!(index.get_rprim(&prim_id).is_none());
        index.insert_rprim(&mesh, &delegate_id, &prim_id);
        assert!(index.get_rprim(&prim_id).is_some());
    }

    #[test]
    fn test_get_sprim_type_check() {
        let mut index = make_index();
        let camera = Token::new("camera");
        let light = Token::new("light");
        let delegate_id = SdfPath::from_string("/D").unwrap();
        let prim_id = SdfPath::from_string("/D/Cam").unwrap();

        index.insert_sprim(&camera, &delegate_id, &prim_id);
        // Correct type returns Some
        assert!(index.get_sprim(&camera, &prim_id).is_some());
        // Wrong type returns None
        assert!(index.get_sprim(&light, &prim_id).is_none());
    }

    #[test]
    fn test_get_bprim_type_check() {
        let mut index = make_index();
        let rb = Token::new("renderBuffer");
        let tex = Token::new("texture");
        let delegate_id = SdfPath::from_string("/D").unwrap();
        let prim_id = SdfPath::from_string("/D/RB").unwrap();

        index.insert_bprim(&rb, &delegate_id, &prim_id);
        assert!(index.get_bprim(&rb, &prim_id).is_some());
        assert!(index.get_bprim(&tex, &prim_id).is_none());
    }

    #[test]
    fn test_rprim_subtree() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let d = SdfPath::from_string("/D").unwrap();

        let m1 = SdfPath::from_string("/D/World/M1").unwrap();
        let m2 = SdfPath::from_string("/D/World/M2").unwrap();
        let m3 = SdfPath::from_string("/D/Other/M3").unwrap();

        index.insert_rprim(&mesh, &d, &m1);
        index.insert_rprim(&mesh, &d, &m2);
        index.insert_rprim(&mesh, &d, &m3);

        let world = SdfPath::from_string("/D/World").unwrap();
        let subtree = index.get_rprim_subtree(&world);
        assert_eq!(subtree.len(), 2);
        assert!(subtree.contains(&m1));
        assert!(subtree.contains(&m2));
    }

    #[test]
    fn test_render_tag_default() {
        let index = make_index();
        let id = SdfPath::from_string("/D/Mesh").unwrap();
        // Default render tag is "geometry"
        assert_eq!(index.get_render_tag(&id), Token::new("geometry"));
    }

    #[test]
    fn test_render_tag_custom() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let d = SdfPath::from_string("/D").unwrap();
        let id = SdfPath::from_string("/D/Mesh").unwrap();

        index.insert_rprim(&mesh, &d, &id);
        index.set_render_tag(&id, Token::new("guide"));
        assert_eq!(index.get_render_tag(&id), Token::new("guide"));

        // Removing rprim clears the tag
        index.remove_rprim(&id);
        assert_eq!(index.get_render_tag(&id), Token::new("geometry"));
    }

    #[test]
    fn test_scene_delegate_for_rprim() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let d = SdfPath::from_string("/Delegate").unwrap();
        let id = SdfPath::from_string("/Delegate/Mesh").unwrap();

        index.insert_rprim(&mesh, &d, &id);
        assert_eq!(index.get_scene_delegate_for_rprim(&id), Some(&d));

        let missing = SdfPath::from_string("/Missing").unwrap();
        assert!(index.get_scene_delegate_for_rprim(&missing).is_none());
    }

    #[test]
    fn test_allocate_rprim_id() {
        let mut index = make_index();
        // Prim IDs are allocated during insert_rprim, verify via rprim_prim_id_map length
        let mesh = Token::new("mesh");
        let d = SdfPath::from_string("/D").unwrap();
        let m1 = SdfPath::from_string("/D/M1").unwrap();
        let m2 = SdfPath::from_string("/D/M2").unwrap();
        let m3 = SdfPath::from_string("/D/M3").unwrap();

        index.insert_rprim(&mesh, &d, &m1);
        index.insert_rprim(&mesh, &d, &m2);
        index.insert_rprim(&mesh, &d, &m3);

        // Verify reverse lookup
        assert_eq!(index.get_rprim_path_from_prim_id(0), m1);
        assert_eq!(index.get_rprim_path_from_prim_id(1), m2);
        assert_eq!(index.get_rprim_path_from_prim_id(2), m3);
        assert!(index.get_rprim_path_from_prim_id(3).is_empty());
        assert!(index.get_rprim_path_from_prim_id(-1).is_empty());
    }

    #[test]
    fn test_get_prim_id_for_rprim_path_with_tombstones() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let d = SdfPath::from_string("/D").unwrap();
        let m1 = SdfPath::from_string("/D/M1").unwrap();
        let m2 = SdfPath::from_string("/D/M2").unwrap();
        let m3 = SdfPath::from_string("/D/M3").unwrap();

        index.insert_rprim(&mesh, &d, &m1);
        index.insert_rprim(&mesh, &d, &m2);
        index.insert_rprim(&mesh, &d, &m3);

        assert_eq!(index.get_prim_id_for_rprim_path(&m1), Some(0));
        assert_eq!(index.get_prim_id_for_rprim_path(&m2), Some(1));
        assert_eq!(index.get_prim_id_for_rprim_path(&m3), Some(2));

        // Leave a tombstone in prim-id map and verify remaining IDs stay stable.
        index.remove_rprim(&m2);
        assert_eq!(index.get_prim_id_for_rprim_path(&m1), Some(0));
        assert_eq!(index.get_prim_id_for_rprim_path(&m3), Some(2));
        assert_eq!(index.get_rprim_path_from_prim_id(1), SdfPath::empty());
    }

    #[test]
    fn test_sprim_ids_for_type() {
        let mut index = make_index();
        let camera = Token::new("camera");
        let light = Token::new("light");
        let d = SdfPath::from_string("/D").unwrap();

        let cam1 = SdfPath::from_string("/D/Cam1").unwrap();
        let cam2 = SdfPath::from_string("/D/Cam2").unwrap();
        let light1 = SdfPath::from_string("/D/Light1").unwrap();

        index.insert_sprim(&camera, &d, &cam1);
        index.insert_sprim(&camera, &d, &cam2);
        index.insert_sprim(&light, &d, &light1);

        let cam_ids = index.get_sprim_ids_for_type(&camera);
        assert_eq!(cam_ids.len(), 2);

        let light_ids = index.get_sprim_ids_for_type(&light);
        assert_eq!(light_ids.len(), 1);
    }

    #[test]
    fn test_rprim_ids_sorted() {
        let mut index = make_index();
        let mesh = Token::new("mesh");
        let d = SdfPath::from_string("/D").unwrap();

        let b = SdfPath::from_string("/D/B").unwrap();
        let a = SdfPath::from_string("/D/A").unwrap();
        let c = SdfPath::from_string("/D/C").unwrap();

        index.insert_rprim(&mesh, &d, &b);
        index.insert_rprim(&mesh, &d, &a);
        index.insert_rprim(&mesh, &d, &c);

        let ids = index.get_rprim_ids();
        assert_eq!(ids[0], a);
        assert_eq!(ids[1], b);
        assert_eq!(ids[2], c);
    }
}
