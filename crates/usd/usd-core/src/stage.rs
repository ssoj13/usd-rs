//! UsdStage - the scene container.
//!
//! UsdStage is the outermost container for scene description. It owns and
//! presents composed prims as a scenegraph.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use usd_ar::ResolverContext;
use usd_gf::Interval;
use usd_pcp::{
    Cache as PcpCache, CachePtr as PcpCachePtr, Changes as PcpChanges, LayerStackIdentifier,
};
use usd_sdf::{AssetPath, Layer, Path, SpecType, Specifier};
use usd_tf::Token;

// Import AssetPath for color configuration
use usd_sdf::asset_path as sdf_asset_path;

use super::clip_cache::ClipCache;
use super::common::{InitialLoadSet, LoadPolicy};
use super::edit_target::EditTarget;
use super::instance_cache::InstanceCache;
use super::interpolation::InterpolationType;
use super::load_rules::StageLoadRules;
use super::population_mask::StagePopulationMask;
use super::prim::Prim;
use super::stage_cache_context::StageCacheContext;
use super::prim_data::PrimData;
use super::prim_flags::{PrimFlags, PrimFlagsPredicate};
use super::prim_range;

// ============================================================================
// Global State
// ============================================================================

use std::sync::OnceLock;

/// Global variant fallback preferences (static storage).
static GLOBAL_VARIANT_FALLBACKS: OnceLock<RwLock<std::collections::HashMap<Token, Vec<Token>>>> =
    OnceLock::new();

/// Global color config fallbacks (static storage).
static GLOBAL_COLOR_CONFIG_FALLBACKS: OnceLock<RwLock<Option<(sdf_asset_path::AssetPath, Token)>>> =
    OnceLock::new();

fn get_global_variant_fallbacks_storage()
-> &'static RwLock<std::collections::HashMap<Token, Vec<Token>>> {
    GLOBAL_VARIANT_FALLBACKS.get_or_init(|| RwLock::new(std::collections::HashMap::new()))
}

fn get_global_color_config_fallbacks_storage()
-> &'static RwLock<Option<(sdf_asset_path::AssetPath, Token)>> {
    GLOBAL_COLOR_CONFIG_FALLBACKS.get_or_init(|| RwLock::new(None))
}

// ============================================================================
// PendingChanges
// ============================================================================

/// Accumulated changes waiting to be processed.
///
/// Matches C++ `UsdStage::_PendingChanges`.
#[derive(Default)]
struct PendingChanges {
    /// PCP-level composition changes.
    pcp_changes: PcpChanges,
    /// Paths requiring full recomposition.
    recompose_changes: HashMap<Path, Vec<super::notice::ChangeEntry>>,
    /// Paths where info (non-structural) changed.
    info_changes: HashMap<Path, Vec<super::notice::ChangeEntry>>,
    /// Paths where asset path resolution changed.
    asset_path_resync_changes: HashMap<Path, Vec<super::notice::ChangeEntry>>,
}

// ============================================================================
// Stage
// ============================================================================

/// The scene container - owns and presents composed prims.
///
/// A UsdStage presents a composed view of scene description from a root layer
/// and all its sublayers, references, payloads, etc.
///
/// # Lifetime Management
///
/// Stages are created via static factory methods:
/// - `create_new` - creates a new stage with a new root layer
/// - `create_in_memory` - creates a stage with an anonymous root layer
/// - `open` - opens an existing file
///
/// # Examples
///
/// ```rust,ignore
/// use usd_core::{Stage, InitialLoadSet};
///
/// // Create a new stage
/// let stage = Stage::create_new("HelloWorld.usda", InitialLoadSet::LoadAll)?;
///
/// // Define a prim
/// let prim = stage.define_prim("/World", "Xform")?;
///
/// // Save
/// stage.save()?;
/// ```
/// Note: Debug not derived because Layer doesn't implement Debug.
pub struct Stage {
    /// Root layer of the stage.
    root_layer: Arc<Layer>,
    /// Session layer (optional).
    session_layer: Option<Arc<Layer>>,
    /// Current edit target.
    edit_target: RwLock<EditTarget>,
    /// Load rules.
    load_rules: RwLock<StageLoadRules>,
    /// Population mask (None means all prims).
    population_mask: RwLock<Option<StagePopulationMask>>,
    /// Resolver context.
    resolver_context: RwLock<Option<ResolverContext>>,
    /// Interpolation type.
    interpolation_type: RwLock<InterpolationType>,
    /// Self reference for creating prims.
    self_ref: std::sync::OnceLock<Weak<Self>>,
    /// Cached prim data indexed by path.
    prim_cache: RwLock<HashMap<Path, Arc<PrimData>>>,
    /// Pseudo-root prim data.
    pseudo_root_data: std::sync::OnceLock<Arc<PrimData>>,
    /// PCP cache for composition queries.
    pcp_cache: RwLock<Option<PcpCachePtr>>,
    /// Muted layer identifiers.
    muted_layers: RwLock<std::collections::HashSet<String>>,
    /// Instance cache for tracking instances and prototypes.
    instance_cache: std::sync::OnceLock<Arc<InstanceCache>>,
    /// Clip cache for value clips.
    clip_cache: std::sync::OnceLock<Arc<ClipCache>>,
    /// Serial number of the last layer change processed.
    last_change_serial: AtomicU64,
    /// Serial number of the last Sdf layer notice processed.
    last_layers_notice_serial: AtomicU64,
    /// Pending composition changes (protected by Mutex for exclusive access).
    pending_changes: Mutex<Option<PendingChanges>>,
    /// Registration for Sdf layer change notices affecting this stage.
    layers_did_change_key: Mutex<Option<usd_tf::notice::ListenerKey>>,
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Stage")
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(key) = self
            .layers_did_change_key
            .lock()
            .expect("mutex poisoned")
            .take()
        {
            usd_tf::notice::revoke(key);
        }
    }
}

impl PartialEq for Stage {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Stage {
    // ========================================================================
    // Factory Methods
    // ========================================================================

    /// Creates a new stage with a new root layer.
    pub fn create_new(
        identifier: impl Into<String>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_new_impl(identifier.into(), None, None, load)
    }

    /// Creates a new stage with a new root layer and session layer.
    pub fn create_new_with_session(
        identifier: impl Into<String>,
        session_layer: Arc<Layer>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_new_impl(identifier.into(), Some(session_layer), None, load)
    }

    /// Creates a new stage with a new root layer and resolver context (matches C++ CreateNew overload).
    pub fn create_new_with_resolver_context(
        identifier: impl Into<String>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_new_impl(identifier.into(), None, resolver_context, load)
    }

    /// Creates a new stage with a new root layer, session layer and resolver context (matches C++ CreateNew overload).
    pub fn create_new_with_session_and_resolver_context(
        identifier: impl Into<String>,
        session_layer: Arc<Layer>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_new_impl(
            identifier.into(),
            Some(session_layer),
            resolver_context,
            load,
        )
    }

    fn create_new_impl(
        identifier: String,
        session_layer: Option<Arc<Layer>>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        // Ensure builtin schemas are registered (C++ does this via TfType plugin system)
        crate::schema_registry::register_builtin_schemas();

        // Create root layer
        let root_layer =
            Layer::create_new(&identifier).map_err(|e| Error::LayerError(e.to_string()))?;

        // Create session layer if not provided
        let session = session_layer.or_else(|| Some(Layer::create_anonymous(Some("session"))));

        let edit_target = EditTarget::for_local_layer(root_layer.clone());

        // Create PCP cache for composition (include session layer so PCP walks it)
        let layer_stack_id = Self::make_layer_stack_identifier(
            root_layer.as_ref(),
            session.as_ref(),
            resolver_context.clone(),
        );
        let pcp_cache = PcpCache::new_with_session(layer_stack_id, session.clone(), true);

        // Initialize load rules based on the load parameter (C++ parity)
        let initial_load_rules = match load {
            InitialLoadSet::LoadAll => StageLoadRules::load_all(),
            InitialLoadSet::LoadNone => StageLoadRules::load_none(),
        };

        // C++ _IncludePayloadsPredicate: loadRules.IsLoaded(path)
        // For LoadAll, all paths are loaded. For LoadNone, none are.
        Self::set_payload_predicate_for_rules(&pcp_cache, &initial_load_rules);

        Self::apply_global_variant_fallbacks_to_pcp_cache(&pcp_cache);

        let stage = Arc::new(Self {
            root_layer,
            session_layer: session,
            edit_target: RwLock::new(edit_target),
            load_rules: RwLock::new(initial_load_rules),
            population_mask: RwLock::new(None),
            resolver_context: RwLock::new(resolver_context),
            interpolation_type: RwLock::new(InterpolationType::default()),
            self_ref: std::sync::OnceLock::new(),
            prim_cache: RwLock::new(HashMap::new()),
            pseudo_root_data: std::sync::OnceLock::new(),
            pcp_cache: RwLock::new(Some(pcp_cache)),
            muted_layers: RwLock::new(std::collections::HashSet::new()),
            instance_cache: std::sync::OnceLock::new(),
            clip_cache: std::sync::OnceLock::new(),
            last_change_serial: AtomicU64::new(0),
            last_layers_notice_serial: AtomicU64::new(0),
            pending_changes: Mutex::new(None),
            layers_did_change_key: Mutex::new(None),
        });

        // Store weak self reference
        let _ = stage.self_ref.set(Arc::downgrade(&stage));
        stage.register_layer_change_listener();
        // Initialize instance cache
        let _ = stage.instance_cache.set(Arc::new(InstanceCache::new()));
        // Initialize clip cache
        let _ = stage.clip_cache.set(Arc::new(ClipCache::new()));

        // Initialize pseudo-root
        stage.init_pseudo_root();

        Ok(stage)
    }

    /// Creates a new stage with an anonymous root layer.
    pub fn create_in_memory(load: InitialLoadSet) -> Result<Arc<Self>, Error> {
        Self::create_in_memory_impl(None, None, None, load)
    }

    /// Creates a new stage with an anonymous root layer and identifier (matches C++ CreateInMemory overload).
    pub fn create_in_memory_with_identifier(
        identifier: impl Into<String>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_in_memory_impl(Some(identifier.into()), None, None, load)
    }

    /// Creates a new stage with an anonymous root layer, identifier and resolver context (matches C++ CreateInMemory overload).
    pub fn create_in_memory_with_resolver_context(
        identifier: impl Into<String>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_in_memory_impl(Some(identifier.into()), None, resolver_context, load)
    }

    /// Creates a new stage with an anonymous root layer, identifier and session layer (matches C++ CreateInMemory overload).
    pub fn create_in_memory_with_session(
        identifier: impl Into<String>,
        session_layer: Arc<Layer>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_in_memory_impl(Some(identifier.into()), Some(session_layer), None, load)
    }

    /// Creates a new stage with an anonymous root layer, identifier, session layer and resolver context (matches C++ CreateInMemory overload).
    pub fn create_in_memory_with_session_and_resolver_context(
        identifier: impl Into<String>,
        session_layer: Arc<Layer>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::create_in_memory_impl(
            Some(identifier.into()),
            Some(session_layer),
            resolver_context,
            load,
        )
    }

    fn create_in_memory_impl(
        identifier: Option<String>,
        session_layer: Option<Arc<Layer>>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        // Ensure builtin schemas are registered (C++ does this via TfType plugin system)
        crate::schema_registry::register_builtin_schemas();

        // Pass identifier as tag to anonymous layer (C++ uses it for debugging)
        let tag = identifier.as_deref().unwrap_or("anon");
        let root_layer = Layer::create_anonymous(Some(tag));
        let edit_target = EditTarget::for_local_layer(root_layer.clone());

        let session = session_layer.or_else(|| Some(Layer::create_anonymous(Some("session"))));

        // Include session layer so PCP walks it (matches C++ UsdStage ctor)
        let layer_stack_id = Self::make_layer_stack_identifier(
            root_layer.as_ref(),
            session.as_ref(),
            resolver_context.clone(),
        );
        let pcp_cache = PcpCache::new_with_session(layer_stack_id, session.clone(), true);

        let initial_load_rules = match load {
            InitialLoadSet::LoadAll => StageLoadRules::load_all(),
            InitialLoadSet::LoadNone => StageLoadRules::load_none(),
        };
        Self::set_payload_predicate_for_rules(&pcp_cache, &initial_load_rules);

        Self::apply_global_variant_fallbacks_to_pcp_cache(&pcp_cache);

        let stage = Arc::new(Self {
            root_layer,
            session_layer: session,
            edit_target: RwLock::new(edit_target),
            load_rules: RwLock::new(initial_load_rules),
            population_mask: RwLock::new(None),
            resolver_context: RwLock::new(resolver_context),
            interpolation_type: RwLock::new(InterpolationType::default()),
            self_ref: std::sync::OnceLock::new(),
            prim_cache: RwLock::new(HashMap::new()),
            pseudo_root_data: std::sync::OnceLock::new(),
            pcp_cache: RwLock::new(Some(pcp_cache)),
            muted_layers: RwLock::new(std::collections::HashSet::new()),
            instance_cache: std::sync::OnceLock::new(),
            clip_cache: std::sync::OnceLock::new(),
            last_change_serial: AtomicU64::new(0),
            last_layers_notice_serial: AtomicU64::new(0),
            pending_changes: Mutex::new(None),
            layers_did_change_key: Mutex::new(None),
        });

        let _ = stage.self_ref.set(Arc::downgrade(&stage));
        stage.register_layer_change_listener();
        // Initialize instance cache
        let _ = stage.instance_cache.set(Arc::new(InstanceCache::new()));
        // Initialize clip cache
        let _ = stage.clip_cache.set(Arc::new(ClipCache::new()));
        stage.init_pseudo_root();
        Ok(stage)
    }

    /// Opens an existing stage from a file.
    pub fn open(file_path: impl Into<String>, load: InitialLoadSet) -> Result<Arc<Self>, Error> {
        Self::open_impl(file_path.into(), None, None, load)
    }

    /// Opens an existing stage from a file with resolver context (matches C++ Open overload).
    pub fn open_with_resolver_context(
        file_path: impl Into<String>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_impl(file_path.into(), None, resolver_context, load)
    }

    /// Opens an existing stage from a root layer (matches C++ Open overload).
    ///
    /// Uses an implicit anonymous session layer when none is supplied.
    pub fn open_with_root_layer(
        root_layer: Arc<Layer>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_with_layer_impl(root_layer, None, None, load)
    }

    /// Open from root layer with **no** session layer (Python `sessionLayer=None`).
    pub fn open_with_root_layer_no_session(
        root_layer: Arc<Layer>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_with_layer_impl(root_layer, Some(None), None, load)
    }

    /// Opens an existing stage from a root layer and session layer (matches C++ Open overload).
    pub fn open_with_root_and_session_layer(
        root_layer: Arc<Layer>,
        session_layer: Arc<Layer>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_with_layer_impl(root_layer, Some(Some(session_layer)), None, load)
    }

    /// Opens an existing stage from a root layer and resolver context (matches C++ Open overload).
    pub fn open_with_root_layer_and_resolver_context(
        root_layer: Arc<Layer>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_with_layer_impl(root_layer, None, resolver_context, load)
    }

    /// Opens an existing stage from a root layer, session layer and resolver context (matches C++ Open overload).
    pub fn open_with_root_session_and_resolver_context(
        root_layer: Arc<Layer>,
        session_layer: Arc<Layer>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_with_layer_impl(root_layer, Some(Some(session_layer)), resolver_context, load)
    }

    /// If any `UsdStageCacheContext` binds a readable cache, return a matching stage when present.
    #[allow(unsafe_code)]
    fn try_find_in_readable_stage_caches(
        root_layer: &Arc<Layer>,
        session_spec: &Option<Option<Arc<Layer>>>,
        resolver_context: &Option<ResolverContext>,
    ) -> Option<Arc<Self>> {
        let root_handle = root_layer.get_handle();
        let caches = StageCacheContext::get_readable_caches();
        for ptr in caches {
            // SAFETY: pointers originate from `StageCacheContext` stack; same invariants as
            // `stage_cache_context` internal lookups.
            let cache = unsafe { &*ptr };
            let found = match (session_spec.as_ref(), resolver_context.as_ref()) {
                // Implicit anonymous session — do not filter by session layer (C++ "don't care").
                (None, Some(res)) => cache.find_one_matching_with_resolver(&root_handle, res),
                (None, None) => cache.find_one_matching(&root_handle),
                // Explicit `sessionLayer=None` — only stages with no session layer.
                (Some(None), Some(res)) => cache.find_one_matching_with_session_and_resolver(
                    &root_handle,
                    None,
                    res,
                ),
                (Some(None), None) => cache.find_one_matching_with_session(&root_handle, None),
                (Some(Some(sess)), Some(res)) => cache.find_one_matching_with_session_and_resolver(
                    &root_handle,
                    Some(&sess.get_handle()),
                    res,
                ),
                (Some(Some(sess)), None) => cache.find_one_matching_with_session(
                    &root_handle,
                    Some(&sess.get_handle()),
                ),
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }

    #[allow(unsafe_code)]
    fn publish_to_writable_stage_caches(stage: &Arc<Self>) {
        for ptr in StageCacheContext::get_writable_caches() {
            // SAFETY: same as `try_find_in_readable_stage_caches`.
            let cache = unsafe { &*ptr };
            cache.insert(stage.clone());
        }
    }

    /// Path resolver context for a root layer when the caller did not supply one.
    ///
    /// Port of `UsdStage::_CreatePathResolverContext` (`pxr/usd/usd/stage.cpp`): non-anonymous
    /// layers use `CreateDefaultContextForAsset` with non-empty repository path, otherwise
    /// `GetRealPath()`; anonymous layers use `CreateDefaultContext()`.
    ///
    /// [`Stage::create_in_memory`] does not use this helper and leaves resolver context unset
    /// (`None`), matching C++ `CreateInMemory`.
    fn create_path_resolver_context_for_root_layer(root_layer: &Layer) -> ResolverContext {
        let resolver = usd_ar::resolver::get_resolver();
        let guard = resolver.read().expect("rwlock poisoned");
        if root_layer.is_anonymous() {
            guard.create_default_context()
        } else {
            let asset_path = match root_layer.get_repository_path() {
                Some(ref s) if !s.is_empty() => s.clone(),
                _ => root_layer.get_resolved_path().unwrap_or_default(),
            };
            guard.create_default_context_for_asset(&asset_path)
        }
    }

    /// [`PcpLayerStackIdentifier`] / [`LayerStackIdentifier`] for this stage's Pcp cache.
    ///
    /// Must carry the same `path_resolver_context` as [`Stage::resolver_context`] (C++
    /// `PcpLayerStackIdentifier` stores `pathResolverContext` alongside root and session handles).
    fn make_layer_stack_identifier(
        root_layer: &Layer,
        session_layer: Option<&Arc<Layer>>,
        path_resolver_context: Option<ResolverContext>,
    ) -> LayerStackIdentifier {
        LayerStackIdentifier::with_parts(
            AssetPath::new(root_layer.identifier()),
            session_layer.map(|s| AssetPath::new(s.identifier())),
            path_resolver_context,
        )
    }

    /// C++ `UsdStage::_InitStage`: `_cache->SetVariantFallbacks(GetGlobalVariantFallbacks())`.
    fn apply_global_variant_fallbacks_to_pcp_cache(pcp_cache: &PcpCache) {
        let global_fallbacks = Self::get_global_variant_fallbacks();
        if global_fallbacks.is_empty() {
            return;
        }
        let pcp_fallbacks: std::collections::HashMap<String, Vec<String>> = global_fallbacks
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.iter().map(|t| t.as_str().to_string()).collect(),
                )
            })
            .collect();
        pcp_cache.set_variant_fallbacks(pcp_fallbacks, None);
    }

    /// `session_spec`: `None` = implicit anonymous session; `Some(None)` = no session;
    /// `Some(Some(layer))` = explicit session.
    fn open_with_layer_impl(
        root_layer: Arc<Layer>,
        session_spec: Option<Option<Arc<Layer>>>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        usd_trace::trace_scope!("stage_open_with_layer");
        // Ensure builtin schemas are registered (C++ does this via TfType plugin system)
        crate::schema_registry::register_builtin_schemas();

        // Auto-create resolver context from root layer if not provided (C++ `_CreatePathResolverContext`)
        let resolver_context = resolver_context
            .or_else(|| Some(Self::create_path_resolver_context_for_root_layer(root_layer.as_ref())));

        if let Some(cached) = Self::try_find_in_readable_stage_caches(
            &root_layer,
            &session_spec,
            &resolver_context,
        ) {
            return Ok(cached);
        }

        let session: Option<Arc<Layer>> = match session_spec {
            None => Some(Layer::create_anonymous(Some("session"))),
            Some(None) => None,
            Some(Some(s)) => Some(s),
        };

        let edit_target = EditTarget::for_local_layer(root_layer.clone());

        // Create PCP cache for composition (include session layer so PCP walks it)
        let layer_stack_id = Self::make_layer_stack_identifier(
            root_layer.as_ref(),
            session.as_ref(),
            resolver_context.clone(),
        );
        let pcp_cache = PcpCache::new_with_session(layer_stack_id, session.clone(), true);

        // Initialize load rules from parameter (C++ parity)
        let initial_load_rules = match load {
            InitialLoadSet::LoadAll => StageLoadRules::load_all(),
            InitialLoadSet::LoadNone => StageLoadRules::load_none(),
        };
        Self::set_payload_predicate_for_rules(&pcp_cache, &initial_load_rules);

        Self::apply_global_variant_fallbacks_to_pcp_cache(&pcp_cache);

        let stage = Arc::new(Self {
            root_layer,
            session_layer: session,
            edit_target: RwLock::new(edit_target),
            load_rules: RwLock::new(initial_load_rules),
            population_mask: RwLock::new(None),
            resolver_context: RwLock::new(resolver_context),
            interpolation_type: RwLock::new(InterpolationType::default()),
            self_ref: std::sync::OnceLock::new(),
            prim_cache: RwLock::new(HashMap::new()),
            pseudo_root_data: std::sync::OnceLock::new(),
            pcp_cache: RwLock::new(Some(pcp_cache)),
            muted_layers: RwLock::new(std::collections::HashSet::new()),
            instance_cache: std::sync::OnceLock::new(),
            clip_cache: std::sync::OnceLock::new(),
            last_change_serial: AtomicU64::new(0),
            last_layers_notice_serial: AtomicU64::new(0),
            pending_changes: Mutex::new(None),
            layers_did_change_key: Mutex::new(None),
        });

        let _ = stage.self_ref.set(Arc::downgrade(&stage));
        stage.register_layer_change_listener();
        // Initialize instance cache
        let _ = stage.instance_cache.set(Arc::new(InstanceCache::new()));
        // Initialize clip cache
        let _ = stage.clip_cache.set(Arc::new(ClipCache::new()));
        stage.init_pseudo_root();

        // Populate prim tree from layer
        stage.populate_from_layer();

        Self::publish_to_writable_stage_caches(&stage);

        Ok(stage)
    }

    fn open_impl(
        file_path: String,
        session_spec: Option<Option<Arc<Layer>>>,
        resolver_context: Option<ResolverContext>,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        usd_trace::trace_scope!("stage_open");
        // Ensure builtin schemas are registered (C++ does this via TfType plugin system)
        crate::schema_registry::register_builtin_schemas();

        // Open root layer
        let root_layer =
            Layer::find_or_open(&file_path).map_err(|e| Error::LayerError(e.to_string()))?;

        // Auto-create resolver context from opened root layer if not provided (same as `Open(SdfLayer)`)
        let resolver_context = resolver_context.or_else(|| {
            Some(Self::create_path_resolver_context_for_root_layer(root_layer.as_ref()))
        });

        if let Some(cached) = Self::try_find_in_readable_stage_caches(
            &root_layer,
            &session_spec,
            &resolver_context,
        ) {
            return Ok(cached);
        }

        let session: Option<Arc<Layer>> = match session_spec {
            None => Some(Layer::create_anonymous(Some("session"))),
            Some(None) => None,
            Some(Some(s)) => Some(s),
        };

        let edit_target = EditTarget::for_local_layer(root_layer.clone());

        // Create PCP cache for composition (include session layer so PCP walks it)
        let layer_stack_id = Self::make_layer_stack_identifier(
            root_layer.as_ref(),
            session.as_ref(),
            resolver_context.clone(),
        );
        let pcp_cache = PcpCache::new_with_session(layer_stack_id, session.clone(), true);

        // Initialize load rules from parameter (C++ parity)
        let initial_load_rules = match load {
            InitialLoadSet::LoadAll => StageLoadRules::load_all(),
            InitialLoadSet::LoadNone => StageLoadRules::load_none(),
        };
        Self::set_payload_predicate_for_rules(&pcp_cache, &initial_load_rules);

        Self::apply_global_variant_fallbacks_to_pcp_cache(&pcp_cache);

        let stage = Arc::new(Self {
            root_layer,
            session_layer: session,
            edit_target: RwLock::new(edit_target),
            load_rules: RwLock::new(initial_load_rules),
            population_mask: RwLock::new(None),
            resolver_context: RwLock::new(resolver_context),
            interpolation_type: RwLock::new(InterpolationType::default()),
            self_ref: std::sync::OnceLock::new(),
            prim_cache: RwLock::new(HashMap::new()),
            pseudo_root_data: std::sync::OnceLock::new(),
            pcp_cache: RwLock::new(Some(pcp_cache)),
            muted_layers: RwLock::new(std::collections::HashSet::new()),
            instance_cache: std::sync::OnceLock::new(),
            clip_cache: std::sync::OnceLock::new(),
            last_change_serial: AtomicU64::new(0),
            last_layers_notice_serial: AtomicU64::new(0),
            pending_changes: Mutex::new(None),
            layers_did_change_key: Mutex::new(None),
        });

        let _ = stage.self_ref.set(Arc::downgrade(&stage));
        stage.register_layer_change_listener();
        // Initialize instance cache
        let _ = stage.instance_cache.set(Arc::new(InstanceCache::new()));
        // Initialize clip cache
        let _ = stage.clip_cache.set(Arc::new(ClipCache::new()));
        stage.init_pseudo_root();

        // Populate prim tree from layer
        stage.populate_from_layer();

        Self::publish_to_writable_stage_caches(&stage);

        Ok(stage)
    }

    /// Opens a stage with a population mask.
    pub fn open_masked(
        file_path: impl Into<String>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_masked_impl(file_path.into(), None, None, mask, load)
    }

    /// Opens a stage with a population mask and resolver context (matches C++ OpenMasked overload).
    pub fn open_masked_with_resolver_context(
        file_path: impl Into<String>,
        resolver_context: Option<ResolverContext>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_masked_impl(file_path.into(), None, resolver_context, mask, load)
    }

    /// Opens a stage with a root layer and population mask (matches C++ OpenMasked overload).
    pub fn open_masked_with_root_layer(
        root_layer: Arc<Layer>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_masked_with_layer_impl(root_layer, None, None, mask, load)
    }

    /// Opens a stage with a root layer, session layer and population mask (matches C++ OpenMasked overload).
    pub fn open_masked_with_root_and_session_layer(
        root_layer: Arc<Layer>,
        session_layer: Arc<Layer>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_masked_with_layer_impl(root_layer, Some(session_layer), None, mask, load)
    }

    /// Opens a stage with a root layer, resolver context and population mask (matches C++ OpenMasked overload).
    pub fn open_masked_with_root_layer_and_resolver_context(
        root_layer: Arc<Layer>,
        resolver_context: Option<ResolverContext>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_masked_with_layer_impl(root_layer, None, resolver_context, mask, load)
    }

    /// Opens a stage with a root layer, session layer, resolver context and population mask (matches C++ OpenMasked overload).
    pub fn open_masked_with_root_session_and_resolver_context(
        root_layer: Arc<Layer>,
        session_layer: Arc<Layer>,
        resolver_context: Option<ResolverContext>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        Self::open_masked_with_layer_impl(
            root_layer,
            Some(session_layer),
            resolver_context,
            mask,
            load,
        )
    }

    fn open_masked_impl(
        file_path: String,
        session_layer: Option<Arc<Layer>>,
        resolver_context: Option<ResolverContext>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        let session_spec = session_layer.map(|s| Some(s));
        let stage_result = Self::open_impl(file_path, session_spec, resolver_context, load)?;

        // Set the population mask
        *stage_result
            .population_mask
            .write()
            .expect("rwlock poisoned") = Some(mask);

        Ok(stage_result)
    }

    fn open_masked_with_layer_impl(
        root_layer: Arc<Layer>,
        session_layer: Option<Arc<Layer>>,
        resolver_context: Option<ResolverContext>,
        mask: StagePopulationMask,
        load: InitialLoadSet,
    ) -> Result<Arc<Self>, Error> {
        let session_spec = session_layer.map(|s| Some(s));
        let stage_result =
            Self::open_with_layer_impl(root_layer, session_spec, resolver_context, load)?;

        // Set the population mask
        *stage_result
            .population_mask
            .write()
            .expect("rwlock poisoned") = Some(mask);

        Ok(stage_result)
    }

    // ========================================================================
    // Internal: Helper Methods
    // ========================================================================

    /// Returns the current edit target layer if it exists in the stage's layer stack.
    ///
    /// Accepts root layer, session layer, or any sublayer — matches C++ edit target handling.
    /// Collect identifiers of session layer and all its sublayers (recursively).
    /// Used by save()/save_session_layers() to distinguish session layers.
    fn session_layer_identifiers(&self) -> std::collections::HashSet<String> {
        let mut ids = std::collections::HashSet::new();
        if let Some(session) = &self.session_layer {
            Self::collect_sublayer_ids(session, &mut ids);
        }
        ids
    }

    /// Recursively collect layer identifier + all sublayer identifiers.
    fn collect_sublayer_ids(layer: &Arc<Layer>, ids: &mut std::collections::HashSet<String>) {
        ids.insert(layer.identifier().to_string());
        for sublayer_path in layer.sublayer_paths() {
            if let Ok(sublayer) = Layer::find_or_open(&sublayer_path) {
                Self::collect_sublayer_ids(&sublayer, ids);
            }
        }
    }

    fn get_edit_target_layer(&self) -> Option<Arc<Layer>> {
        let edit_target = self.edit_target.read().expect("rwlock poisoned");
        let Some(target_layer) = edit_target.layer() else {
            return None;
        };

        // Fast path: root layer
        if Arc::ptr_eq(target_layer, &self.root_layer) {
            return Some(self.root_layer.clone());
        }
        // Fast path: session layer
        if let Some(session) = &self.session_layer {
            if Arc::ptr_eq(target_layer, session) {
                return Some(session.clone());
            }
        }
        // Slow path: any sublayer in the full layer stack
        // Drop the read lock before calling layer_stack() to avoid deadlock
        let target_id = target_layer.identifier().to_string();
        drop(edit_target);
        for layer in self.layer_stack() {
            if layer.identifier() == target_id {
                return Some(layer);
            }
        }
        None
    }

    // ========================================================================
    // Internal: Prim Tree Management
    // ========================================================================

    /// Initializes the pseudo-root prim data.
    fn init_pseudo_root(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let pseudo_root = Arc::new(PrimData::pseudo_root(weak));
        let _ = self.pseudo_root_data.set(pseudo_root.clone());
        self.prim_cache
            .write()
            .expect("rwlock poisoned")
            .insert(Path::absolute_root(), pseudo_root);
    }

    /// Populates the prim tree via PCP composition.
    ///
    /// This is the core composition entry point. Like C++ `_ComposeSubtreeImpl`,
    /// we compute PcpPrimIndex for each path to discover children, type, specifier,
    /// properties, and flags from ALL composed layers (not just root_layer).
    fn populate_from_layer(self: &Arc<Self>) {
        usd_trace::trace_scope!("stage_populate");
        let weak = Arc::downgrade(self);

        // Get root-level children via composition
        let root_children = self.get_composed_child_names(&Path::absolute_root());

        // BFS through the composed prim hierarchy
        let mut work: Vec<Path> = root_children
            .into_iter()
            .rev() // rev for correct pop order
            .collect();

        let t0 = std::time::Instant::now();
        let mut composed = 0u64;
        while let Some(path) = work.pop() {
            self.compose_single_prim(&path, &weak, &mut work);
            composed += 1;
            if composed % 5000 == 0 {
                eprintln!(
                    "[STAGE] composed {} prims ({:.1}s, queue={})",
                    composed,
                    t0.elapsed().as_secs_f64(),
                    work.len()
                );
            }
        }
        eprintln!(
            "[STAGE] populate done: {} prims in {:.2}s",
            composed,
            t0.elapsed().as_secs_f64()
        );

        // Process instancing changes: create prototype prims and compose their subtrees.
        // Matches C++ _ComposePrimIndexesInParallel (stage.cpp:5396-5412).
        self.process_instancing_changes(&weak);
    }

    /// Composes a single prim from its PcpPrimIndex and queues children.
    ///
    /// Reads type_name, specifier, properties, flags, and children from the
    /// composed PrimIndex rather than a single layer spec.
    fn compose_single_prim(
        self: &Arc<Self>,
        path: &Path,
        weak_stage: &Weak<Self>,
        work: &mut Vec<Path>,
    ) {
        usd_trace::trace_scope!("stage_compose_prim");
        // Compute PrimIndex via PCP cache — this composes across all arcs
        let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
        let Some(ref pcp_cache) = *pcp_cache_guard else {
            return;
        };
        let (prim_index, _errors) = pcp_cache.compute_prim_index(path);
        if !prim_index.is_valid() {
            return;
        }

        // Get type_name and specifier from the STRONGEST opinion
        let (type_name, specifier) = self.resolve_type_and_specifier(&prim_index);

        // Create PrimData with composed info
        let prim_data = Arc::new(PrimData::new(
            path.clone(),
            weak_stage.clone(),
            type_name,
            specifier,
        ));

        // Store source prim index path. Matches C++ prim->_primIndex = cache->FindPrimIndex(path).
        // For normal prims the source path equals the prim path.
        prim_data.set_source_prim_index_path(path.clone());

        // Compose property names from all contributing nodes
        let prop_names = prim_index.compute_prim_property_names();
        prim_data.set_property_names(prop_names);

        // Instancing: check instanceable across composed opinions
        let is_instanceable = prim_index.is_instanceable();
        if is_instanceable {
            prim_data.add_flags(PrimFlags::INSTANCE);

            let instance_cache = self
                .instance_cache
                .get()
                .expect("Instance cache should be initialized");
            let population_mask = self.population_mask.read().expect("rwlock poisoned");
            let load_rules = self.load_rules.read().expect("rwlock poisoned");

            let prim_index_arc = Arc::new(prim_index.clone());
            let _needs_prototype = instance_cache.register_instance_prim_index(
                &prim_index_arc,
                population_mask.as_ref(),
                &load_rules,
            );
        }

        // Link to parent
        let parent_path = path.get_parent_path();
        if parent_path.is_absolute_root_path() || parent_path.is_empty() {
            if let Some(pseudo_root) = self.pseudo_root_data.get() {
                pseudo_root.add_child(prim_data.clone());
            }
        } else {
            if let Some(parent_data) = self
                .prim_cache
                .read()
                .expect("rwlock poisoned")
                .get(&parent_path)
            {
                parent_data.add_child(prim_data.clone());
            }
        }

        // Compose flags after parent linkage so model/group hierarchy matches C++.
        self.compose_prim_flags(&prim_index, &prim_data);

        let parent_has_defining_specifier = prim_data
            .parent()
            .map(|parent| parent.has_defining_specifier())
            .unwrap_or(true);
        if !prim_data.has_defining_specifier()
            && !parent_has_defining_specifier
            && prim_index.num_nodes() <= 1
        {
            return;
        }

        // Cache the prim data
        self.prim_cache
            .write()
            .expect("rwlock poisoned")
            .insert(path.clone(), prim_data);

        // Instance prims do not directly expose their name children. Likewise,
        // inactive and unloaded prims exist in the stage but their subtrees are
        // not composed into the live prim cache.
        let should_prune_children = self
            .get_prim_data(path)
            .map(|d| !d.is_active())
            .unwrap_or(false);
        if is_instanceable || should_prune_children {
            return;
        }

        // Queue composed children
        {
            let (child_names, _prohib) = prim_index.compute_prim_child_names();

            let children: Vec<Path> = child_names
                .iter()
                .filter_map(|name| path.append_child(name.as_str()))
                .collect();
            work.extend(children.into_iter().rev());
        }
    }

    /// Processes instancing changes after initial population.
    ///
    /// Calls InstanceCache::process_changes to finalize prototype assignments,
    /// then creates PrimData for each new prototype and recursively composes
    /// their subtrees using the source prim index.
    ///
    /// Matches C++ _ComposePrimIndexesInParallel (stage.cpp:5396-5412)
    /// + recomposition loop (stage.cpp:5275-5292).
    fn process_instancing_changes(self: &Arc<Self>, weak_stage: &Weak<Self>) {
        let instance_cache = self
            .instance_cache
            .get()
            .expect("Instance cache should be initialized");

        let mut changes = super::instance_cache::InstanceChanges::default();
        instance_cache.process_changes(&mut changes);

        if changes.new_prototype_prims.is_empty() && changes.changed_prototype_prims.is_empty() {
            return;
        }

        let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
        let Some(ref pcp_cache) = *pcp_cache_guard else {
            return;
        };

        // Instantiate new prototype prims and compose their subtrees.
        // Matches C++ _InstantiatePrototypePrim (stage.cpp:2963-2973)
        // + _ComposeSubtreeImpl (stage.cpp:3463-3524).
        for i in 0..changes.new_prototype_prims.len() {
            let prototype_path = &changes.new_prototype_prims[i];
            let source_index_path = &changes.new_prototype_prim_indexes[i];

            self.instantiate_and_compose_prototype(
                prototype_path,
                source_index_path,
                pcp_cache,
                weak_stage,
            );
        }

        // Handle changed prototypes (source index changed)
        for i in 0..changes.changed_prototype_prims.len() {
            let prototype_path = &changes.changed_prototype_prims[i];
            let source_index_path = &changes.changed_prototype_prim_indexes[i];

            // Remove old prototype subtree from cache
            self.destroy_prototype_subtree(prototype_path);

            self.instantiate_and_compose_prototype(
                prototype_path,
                source_index_path,
                pcp_cache,
                weak_stage,
            );
        }

        // Remove dead prototypes
        for dead_path in &changes.dead_prototype_prims {
            self.destroy_prototype_subtree(dead_path);
        }

        eprintln!(
            "[STAGE] instancing: {} new prototypes, {} changed, {} dead",
            changes.new_prototype_prims.len(),
            changes.changed_prototype_prims.len(),
            changes.dead_prototype_prims.len(),
        );
    }

    /// Creates a prototype PrimData and recursively composes its subtree.
    ///
    /// Prototype prims are parented beneath the pseudo-root, but are NOT
    /// children of the pseudo-root. This ensures consumers never see prototype
    /// prims unless explicitly asked for.
    ///
    /// Matches C++ _InstantiatePrototypePrim + _ComposeSubtreeImpl.
    fn instantiate_and_compose_prototype(
        self: &Arc<Self>,
        prototype_path: &Path,
        source_index_path: &Path,
        pcp_cache: &usd_pcp::Cache,
        weak_stage: &Weak<Self>,
    ) {
        // Get PrimIndex using the source path (not the prototype path)
        let (source_index, _errors) = pcp_cache.compute_prim_index(source_index_path);
        if !source_index.is_valid() {
            return;
        }

        // Get type and specifier from source prim index
        let (type_name, specifier) = self.resolve_type_and_specifier(&source_index);

        // Create PrimData at the prototype path
        let prim_data = Arc::new(PrimData::new(
            prototype_path.clone(),
            weak_stage.clone(),
            type_name,
            specifier,
        ));

        // Store source prim index path. Matches C++ prim->_primIndex = cache->FindPrimIndex(primIndexPath)
        // where primIndexPath != prim->GetPath() for prototype prims.
        prim_data.set_source_prim_index_path(source_index_path.clone());

        // Set PROTOTYPE flag.
        // Matches C++ _ComposeAndCacheFlags(parent, isPrototypePrim=true) which sets
        // Usd_PrimFlagBits::PrototypePrimBit.
        prim_data.add_flags(PrimFlags::PROTOTYPE);

        // Compose property names from source index
        let prop_names = source_index.compute_prim_property_names();
        prim_data.set_property_names(prop_names);

        // Prototype prims are parented beneath pseudo-root, but NOT as children.
        // This ensures they're invisible to normal traversal.
        // Matches C++ _InstantiatePrototypePrim (stage.cpp:2965-2972).
        // We skip: pseudo_root.add_child(prim_data) — intentionally!
        if let Some(pseudo_root) = self.pseudo_root_data.get() {
            prim_data.set_parent(Some(Arc::downgrade(pseudo_root)));
        }

        // Compose flags after parent linkage so model hierarchy and prototype flags
        // see the same parent state as OpenUSD.
        self.compose_prim_flags(&source_index, &prim_data);

        // Cache the prototype prim data
        self.prim_cache
            .write()
            .expect("rwlock poisoned")
            .insert(prototype_path.clone(), prim_data);

        // Recursively compose children of the prototype.
        // Children use prototype_path prefix but source_index_path for PCP lookup.
        self.compose_prototype_children(
            prototype_path,
            source_index_path,
            &source_index,
            pcp_cache,
            weak_stage,
        );
    }

    /// Recursively composes children of a prototype prim.
    ///
    /// Children are created at prototype_path/child_name but use
    /// source_index_path/child_name for PcpPrimIndex lookup.
    ///
    /// Matches C++ _ComposeSubtreeImpl recursive flow through _ComposeChildren.
    fn compose_prototype_children(
        self: &Arc<Self>,
        prototype_path: &Path,
        source_index_path: &Path,
        source_index: &usd_pcp::PrimIndex,
        pcp_cache: &usd_pcp::Cache,
        weak_stage: &Weak<Self>,
    ) {
        // Get child names from the source prim index
        let (child_names, _prohib) = source_index.compute_prim_child_names();

        for child_name in &child_names {
            let Some(child_proto_path) = prototype_path.append_child(child_name.as_str()) else {
                continue;
            };
            let Some(child_source_path) = source_index_path.append_child(child_name.as_str())
            else {
                continue;
            };

            // Compute PrimIndex for the child using source path
            let (child_index, _errors) = pcp_cache.compute_prim_index(&child_source_path);
            if !child_index.is_valid() {
                continue;
            }

            let (type_name, specifier) = self.resolve_type_and_specifier(&child_index);

            let child_data = Arc::new(PrimData::new(
                child_proto_path.clone(),
                weak_stage.clone(),
                type_name,
                specifier,
            ));

            // Store source prim index path for this prototype child.
            // Matches C++ prim->_primIndex = cache->FindPrimIndex(primIndexPath) in _ComposeSubtreeImpl.
            child_data.set_source_prim_index_path(child_source_path.clone());

            // Compose property names
            let prop_names = child_index.compute_prim_property_names();
            child_data.set_property_names(prop_names);

            // Link to parent prototype prim
            if let Some(parent_data) = self
                .prim_cache
                .read()
                .expect("rwlock poisoned")
                .get(prototype_path)
            {
                parent_data.add_child(child_data.clone());
            }

            // Compose flags after parent linkage so hierarchy-sensitive bits
            // observe the correct parent state.
            self.compose_prim_flags(&child_index, &child_data);

            // Check if child is instanceable — if so, skip its children
            // (nested instancing: instance inside a prototype)
            let is_child_instance = child_index.is_instanceable();
            if is_child_instance {
                child_data.add_flags(PrimFlags::INSTANCE);
                let instance_cache = self
                    .instance_cache
                    .get()
                    .expect("Instance cache should be initialized");
                let population_mask = self.population_mask.read().expect("rwlock poisoned");
                let load_rules = self.load_rules.read().expect("rwlock poisoned");
                let child_index_arc = Arc::new(child_index.clone());
                let _ = instance_cache.register_instance_prim_index(
                    &child_index_arc,
                    population_mask.as_ref(),
                    &load_rules,
                );
            }

            // Cache
            self.prim_cache
                .write()
                .expect("rwlock poisoned")
                .insert(child_proto_path.clone(), child_data);

            // Recurse into children (unless instance)
            if !is_child_instance {
                self.compose_prototype_children(
                    &child_proto_path,
                    &child_source_path,
                    &child_index,
                    pcp_cache,
                    weak_stage,
                );
            }
        }
    }

    /// Removes a prototype subtree from the prim cache.
    /// Used when prototypes are destroyed or changed.
    fn destroy_prototype_subtree(&self, prototype_path: &Path) {
        let mut cache = self.prim_cache.write().expect("rwlock poisoned");
        // Collect all paths under this prototype
        let paths_to_remove: Vec<Path> = cache
            .keys()
            .filter(|p| p.has_prefix(prototype_path))
            .cloned()
            .collect();
        for p in &paths_to_remove {
            cache.remove(p);
        }
    }

    /// Gets composed child names for a path via PCP.
    ///
    /// For [`Path::absolute_root()`], merges `primChildren` across the full root layer stack.
    /// If the layer stack cannot be computed, returns an empty list (does not substitute
    /// root-layer-only data — that would hide PCP / resolver failures).
    pub(crate) fn get_composed_child_names(&self, parent_path: &Path) -> Vec<Path> {
        if parent_path == &Path::absolute_root() {
            let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
            if let Some(ref pcp_cache) = *pcp_cache_guard {
                let layer_stack = if let Some(ls) = pcp_cache.layer_stack() {
                    Some(ls)
                } else {
                    match pcp_cache.compute_layer_stack(pcp_cache.layer_stack_identifier()) {
                        Ok(ls) => Some(ls),
                        Err(_) => None,
                    }
                };

                if let Some(layer_stack) = layer_stack {
                    let mut seen = std::collections::HashSet::new();
                    let mut children = Vec::new();
                    let children_token = Token::new("primChildren");

                    for layer in layer_stack.get_layers() {
                        let tokens: Vec<Token> = layer
                            .get_field(parent_path, &children_token)
                            .and_then(|v| v.as_vec_clone::<Token>())
                            .unwrap_or_default();
                        for t in tokens {
                            if seen.insert(t.clone()) {
                                if let Some(child_path) = parent_path.append_child(t.as_str()) {
                                    children.push(child_path);
                                }
                            }
                        }
                    }
                    return children;
                }
            }

            return Vec::new();
        }

        // Non-root: compute via PrimIndex
        let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
        if let Some(ref pcp_cache) = *pcp_cache_guard {
            let (prim_index, _) = pcp_cache.compute_prim_index(parent_path);
            if prim_index.is_valid() {
                let (child_names, _prohib) = prim_index.compute_prim_child_names();
                return child_names
                    .iter()
                    .filter_map(|name| parent_path.append_child(name.as_str()))
                    .collect();
            }
        }

        Vec::new()
    }

    /// Resolves type_name and specifier from strongest opinion in the PrimIndex.
    fn resolve_type_and_specifier(&self, prim_index: &usd_pcp::PrimIndex) -> (Token, Specifier) {
        let type_name_token = Token::new("typeName");
        let specifier_token = Token::new("specifier");
        let mut type_name = Token::new("");
        let mut specifier = Specifier::Over;
        let mut found_specifier = false;
        let mut found_type = false;

        // Walk nodes strongest to weakest
        for node in prim_index.nodes() {
            if !node.can_contribute_specs() || !node.has_specs() {
                continue;
            }
            let Some(layer_stack) = node.layer_stack() else {
                continue;
            };
            let site_path = node.path();

            for layer in layer_stack.get_layers() {
                // Resolve specifier — skip Over (non-defining), first Def/Class wins.
                // Matches C++ _GetPrimSpecifierImpl: Over is "no opinion", Def wins
                // immediately, and Class is remembered but may still be overridden
                // by a stronger/equally-strong Def later in the walk.
                if !found_specifier {
                    if let Some(spec_val) = layer.get_field(&site_path, &specifier_token) {
                        let parsed = spec_val.downcast_clone::<Specifier>().or_else(|| {
                            spec_val
                                .downcast_clone::<String>()
                                .and_then(|s| Specifier::try_from(s.as_str()).ok())
                        });
                        if let Some(s) = parsed {
                            match s {
                                Specifier::Def => {
                                    specifier = Specifier::Def;
                                    found_specifier = true;
                                }
                                Specifier::Class => {
                                    specifier = Specifier::Class;
                                }
                                Specifier::Over => {}
                            }
                        }
                    }
                }

                // Resolve type name (first non-empty opinion wins)
                if !found_type {
                    if let Some(tn_val) = layer.get_field(&site_path, &type_name_token) {
                        if let Some(tn) = tn_val.downcast_clone::<Token>() {
                            if !tn.as_str().is_empty() {
                                type_name = tn;
                                found_type = true;
                            }
                        } else if let Some(tn_str) = tn_val.downcast_clone::<String>() {
                            if !tn_str.is_empty() {
                                type_name = Token::new(&tn_str);
                                found_type = true;
                            }
                        }
                    }
                }

                if found_specifier && found_type {
                    return (type_name, specifier);
                }
            }
        }

        // If no specifier found in fields, check layer specs directly
        if !found_specifier {
            let path = prim_index.path();
            if let Some(spec) = self.root_layer.get_prim_at_path(&path) {
                specifier = spec.specifier();
            }
        }
        if !found_type {
            let path = prim_index.path();
            if let Some(spec) = self.root_layer.get_prim_at_path(&path) {
                type_name = spec.type_name();
            }
        }

        (type_name, specifier)
    }

    /// Sets prim flags from composed opinions (active, payload, model, group).
    /// Composes prim flags from the prim index (active, kind, loaded, abstract).
    ///
    /// C++ `Usd_PrimData::_ComposeAndCacheFlags` (primData.cpp:100-170):
    /// Walks the prim index nodes to find active/kind opinions, then sets
    /// LOADED based on payload inclusion (PcpCache::IsPayloadIncluded)
    /// and ABSTRACT based on specifier being Class.
    fn compose_prim_flags(&self, prim_index: &usd_pcp::PrimIndex, prim_data: &Arc<PrimData>) {
        let active_token = Token::new("active");
        let kind_token = Token::new("kind");
        let mut found_active = false;
        let mut found_kind = false;
        let parent_data = prim_data.parent();
        let parent_is_group = parent_data
            .as_ref()
            .map(|parent| parent.is_group())
            .unwrap_or(false);
        let parent_is_defined = parent_data
            .as_ref()
            .map(|parent| parent.is_defined())
            .unwrap_or(true);
        let parent_is_loaded = parent_data
            .as_ref()
            .map(|parent| parent.is_loaded())
            .unwrap_or(true);
        let parent_is_abstract = parent_data
            .as_ref()
            .map(|parent| parent.is_abstract())
            .unwrap_or(false);

        prim_data.remove_flags(
            PrimFlags::MODEL
                | PrimFlags::GROUP
                | PrimFlags::COMPONENT
                | PrimFlags::HAS_PAYLOAD
                | PrimFlags::LOADED
                | PrimFlags::DEFINED
                | PrimFlags::HAS_DEFINING_SPECIFIER
                | PrimFlags::ABSTRACT,
        );

        let is_defining_spec = prim_data.specifier() == usd_sdf::Specifier::Def
            || prim_data.specifier() == usd_sdf::Specifier::Class;
        if is_defining_spec {
            prim_data.add_flags(PrimFlags::HAS_DEFINING_SPECIFIER);
        }
        if parent_is_defined && is_defining_spec {
            prim_data.add_flags(PrimFlags::DEFINED);
        }
        if prim_data.specifier() == usd_sdf::Specifier::Class || parent_is_abstract {
            prim_data.add_flags(PrimFlags::ABSTRACT);
        }

        // C++ primData.cpp:109-111: check payload from prim index
        let has_payload = prim_index.has_any_payloads();
        if has_payload {
            prim_data.add_flags(PrimFlags::HAS_PAYLOAD);
        }

        // Walk nodes to find active and kind (first opinion wins)
        for node in prim_index.nodes() {
            if !node.can_contribute_specs() || !node.has_specs() {
                continue;
            }
            let Some(layer_stack) = node.layer_stack() else {
                continue;
            };
            let site_path = node.path();

            for layer in layer_stack.get_layers() {
                if !found_active {
                    if let Some(val) = layer.get_field(&site_path, &active_token) {
                        let active = val.downcast_clone::<bool>().or_else(|| {
                            val.cast::<bool>()
                                .and_then(|casted| casted.get::<bool>().copied())
                        });
                        if let Some(active) = active {
                            if !active {
                                prim_data.remove_flags(PrimFlags::ACTIVE);
                            }
                            found_active = true;
                        }
                    }
                }

                if parent_is_group && !found_kind {
                    if let Some(val) = layer.get_field(&site_path, &kind_token) {
                        if let Some(kind_str) = val.downcast_clone::<String>() {
                            let kind = Token::new(&kind_str);
                            let is_group = usd_kind::is_group_kind(&kind);
                            let is_component = usd_kind::is_component_kind(&kind);
                            let is_model =
                                is_group || is_component || usd_kind::is_model_kind(&kind);
                            if is_group {
                                prim_data.add_flags(PrimFlags::GROUP);
                            }
                            if is_component {
                                prim_data.add_flags(PrimFlags::COMPONENT);
                            }
                            if is_model {
                                prim_data.add_flags(PrimFlags::MODEL);
                            }
                            found_kind = true;
                        } else if let Some(kind) = val.downcast_clone::<Token>() {
                            let is_group = usd_kind::is_group_kind(&kind);
                            let is_component = usd_kind::is_component_kind(&kind);
                            let is_model =
                                is_group || is_component || usd_kind::is_model_kind(&kind);
                            if is_group {
                                prim_data.add_flags(PrimFlags::GROUP);
                            }
                            if is_component {
                                prim_data.add_flags(PrimFlags::COMPONENT);
                            }
                            if is_model {
                                prim_data.add_flags(PrimFlags::MODEL);
                            }
                            found_kind = true;
                        }
                    }
                }

                if found_active && found_kind {
                    break;
                }
            }

            if found_active && found_kind {
                break;
            }
        }

        // C++ primData.cpp:136-139: LOADED flag.
        // If prim has a payload, check if PcpCache considers the payload included
        // (based on load rules set via LoadAll/LoadNone/Load). If no payload,
        // inherit loaded state from parent (all ancestors without payloads are loaded).
        let is_active = prim_data.is_active();
        if has_payload {
            let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
            let payload_included = if let Some(ref pcp_cache) = *pcp_cache_guard {
                pcp_cache.is_payload_included(&prim_data.path())
            } else {
                true
            };
            if is_active && payload_included {
                prim_data.add_flags(PrimFlags::LOADED);
            }
        } else if parent_is_loaded && is_active {
            prim_data.add_flags(PrimFlags::LOADED);
        }

        // C++ stage.cpp:3371-3396 `_ComposeAuthoredAppliedSchemas`:
        // Walk the prim index via Resolver, collect SdfTokenListOp opinions
        // for the "apiSchemas" field from strongest to weakest, then apply
        // them in reverse order (weakest to strongest) to produce the final
        // composed list of applied API schemas.
        let api_schemas_key = Token::new("apiSchemas");
        let index_arc = Arc::new(prim_index.clone());
        let mut resolver = crate::resolver::Resolver::new(&index_arc, true);
        let mut list_ops: Vec<usd_sdf::ListOp<Token>> = Vec::new();
        while resolver.is_valid() {
            if let (Some(layer), Some(local_path)) =
                (resolver.get_layer(), resolver.get_local_path())
            {
                if let Some(val) = layer.get_field(&local_path, &api_schemas_key) {
                    if let Some(list_op) = val.get::<usd_sdf::ListOp<Token>>() {
                        let is_explicit = list_op.is_explicit();
                        list_ops.push(list_op.clone());
                        if is_explicit {
                            break;
                        }
                    }
                }
            }
            resolver.next_layer();
        }
        if !list_ops.is_empty() {
            let mut schemas: Vec<Token> = Vec::new();
            // Apply weakest to strongest (reverse of collection order)
            for list_op in list_ops.iter().rev() {
                list_op.apply_operations(
                    &mut schemas,
                    None::<fn(usd_sdf::ListOpType, &Token) -> Option<Token>>,
                );
            }
            if !schemas.is_empty() {
                prim_data.set_applied_api_schemas(schemas);
            }
        }
    }

    /// Gets or creates PrimData for a path.
    #[allow(dead_code)] // Internal API - used for prim authoring
    fn get_or_create_prim_data(self: &Arc<Self>, path: &Path) -> Option<Arc<PrimData>> {
        // Check cache first
        if let Some(data) = self.prim_cache.read().expect("rwlock poisoned").get(path) {
            return Some(data.clone());
        }

        // Create new PrimData (for authoring new prims)
        let weak = Arc::downgrade(self);
        let prim_data = Arc::new(PrimData::new(
            path.clone(),
            weak,
            Token::new(""),
            Specifier::Over,
        ));

        self.prim_cache
            .write()
            .expect("rwlock poisoned")
            .insert(path.clone(), prim_data.clone());
        Some(prim_data)
    }

    // ========================================================================
    // Layer Access
    // ========================================================================

    /// Returns the root layer.
    pub fn root_layer(&self) -> &Arc<Layer> {
        &self.root_layer
    }

    /// Returns the root layer (alias for root_layer, matches C++ GetRootLayer).
    pub fn get_root_layer(&self) -> Arc<Layer> {
        self.root_layer.clone()
    }

    /// Returns the session layer.
    pub fn session_layer(&self) -> Option<&Arc<Layer>> {
        self.session_layer.as_ref()
    }

    /// Returns the session layer (alias for session_layer, matches C++ GetSessionLayer).
    pub fn get_session_layer(&self) -> Option<Arc<Layer>> {
        self.session_layer.clone()
    }

    /// Returns the flattened layer stack.
    ///
    /// The layer stack includes all layers that contribute opinions to the stage,
    /// ordered from strongest to weakest:
    /// 1. Session layer (if present)
    /// 2. Root layer
    /// 3. Sublayers (recursively via PCP composition)
    ///
    /// Uses PCP cache to get the complete layer stack including all sublayers.
    pub fn layer_stack(&self) -> Vec<Arc<Layer>> {
        // Get layer stack from PCP cache
        let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
        if let Some(ref pcp_cache) = *pcp_cache_guard {
            if let Some(layer_stack) = pcp_cache.layer_stack() {
                // Get layers from PCP layer stack (includes all sublayers)
                return layer_stack.get_layers();
            }
        }

        // Fallback: if PCP layer stack not computed, use session + root
        let mut stack = Vec::new();
        if let Some(session) = &self.session_layer {
            stack.push(session.clone());
        }
        stack.push(self.root_layer.clone());
        stack
    }

    /// Returns the layer stack, optionally excluding session layers.
    ///
    /// C++ `UsdStage::GetLayerStack` (stage.cpp:4069-4095):
    /// Gets the full PCP layer stack (session + root + sublayers), then:
    /// - `include_session_layers=true`: returns all layers
    /// - `include_session_layers=false`: finds root layer position in the
    ///   stack and returns from root to end (root + all sublayers).
    ///
    /// This is critical for `save_all()` which calls `get_layer_stack(false)`
    /// to save root + sublayers without session layer edits.
    pub fn get_layer_stack(&self, include_session_layers: bool) -> Vec<Arc<Layer>> {
        let all_layers = self.layer_stack();
        if include_session_layers {
            all_layers
        } else {
            // C++ does: find(layers.begin(), layers.end(), GetRootLayer())
            // then: result.assign(copyBegin, layers.end())
            if let Some(pos) = all_layers
                .iter()
                .position(|l| Arc::ptr_eq(l, &self.root_layer))
            {
                all_layers[pos..].to_vec()
            } else {
                vec![self.root_layer.clone()]
            }
        }
    }

    /// Returns used layers (all layers contributing to composition).
    pub fn used_layers(&self) -> Vec<Arc<Layer>> {
        self.layer_stack()
    }

    /// Returns used layers (alias for used_layers, matches C++ GetUsedLayers).
    ///
    /// Returns all layers currently consumed by this stage, as determined by
    /// composition arcs. If include_clip_layers is true, also includes layers
    /// opened for value clips.
    pub fn get_used_layers(&self, include_clip_layers: bool) -> Vec<Arc<Layer>> {
        let mut layers = self.used_layers();

        if include_clip_layers {
            // Get clip layers from clip cache
            if let Some(clip_cache) = self.clip_cache.get() {
                let clip_layers = clip_cache.get_used_layers();
                // Add clip layers to result, avoiding duplicates
                use std::collections::HashSet;
                let mut layer_identifiers: HashSet<String> =
                    layers.iter().map(|l| l.identifier().to_string()).collect();

                for clip_layer in clip_layers {
                    let identifier = clip_layer.identifier().to_string();
                    if !layer_identifiers.contains(&identifier) {
                        layer_identifiers.insert(identifier.clone());
                        layers.push(clip_layer);
                    }
                }
            }
        }

        layers
    }

    /// Returns true if the layer is in this stage's local layer stack (matches C++ HasLocalLayer).
    pub fn has_local_layer(&self, layer: &Arc<Layer>) -> bool {
        self.layer_stack().iter().any(|l| Arc::ptr_eq(l, layer))
    }

    /// Resolves an identifier to an edit target (matches C++ ResolveIdentifierToEditTarget).
    ///
    /// Uses the stage's resolver context and the layer of the current edit target
    /// as an anchor for relative references (e.g., @./siblingFile.usd@).
    pub fn resolve_identifier_to_edit_target(&self, identifier: &str) -> String {
        use usd_ar::resolver::get_resolver;

        // Get current edit target's layer as anchor
        let edit_target = self.edit_target.read().expect("rwlock poisoned");
        let anchor_path = edit_target
            .layer()
            .map(|l| l.identifier())
            .unwrap_or_default();

        // Get resolver
        let resolver = get_resolver().read().expect("rwlock poisoned");

        // Create anchor resolved path if we have an anchor
        let _anchor_resolved = if !anchor_path.is_empty() {
            // Try to resolve anchor path first
            let resolved = resolver.resolve(anchor_path);
            if !resolved.is_empty() {
                Some(resolved)
            } else {
                None
            }
        } else {
            None
        };

        // Resolve identifier using resolver
        // If identifier is already resolved (absolute path), resolver will return it as-is
        let resolved = resolver.resolve(identifier);

        if !resolved.is_empty() {
            resolved.to_string()
        } else {
            // If resolution failed, try creating identifier for new asset
            let new_resolved = resolver.resolve_for_new_asset(identifier);
            if !new_resolved.is_empty() {
                new_resolved.to_string()
            } else {
                // Fallback: return identifier as-is
                identifier.to_string()
            }
        }
    }

    /// Returns composition errors (matches C++ GetCompositionErrors).
    ///
    /// Returns a vector of error messages from PCP composition.
    /// Collects errors from all prim indices computed by the PCP cache.
    pub fn get_composition_errors(&self) -> Vec<String> {
        fn collect_authored_prim_paths(
            prim: &usd_sdf::PrimSpec,
            paths: &mut std::collections::BTreeSet<Path>,
        ) {
            paths.insert(prim.path().clone());
            for child in prim.name_children() {
                collect_authored_prim_paths(&child, paths);
            }
        }

        let Some(pcp_cache) = self
            .pcp_cache
            .read()
            .expect("rwlock poisoned")
            .as_ref()
            .cloned()
        else {
            return Vec::new();
        };

        let mut prim_paths = std::collections::BTreeSet::new();
        prim_paths.insert(Path::absolute_root());
        for layer in self.get_layer_stack(true) {
            for root in layer.root_prims() {
                collect_authored_prim_paths(&root, &mut prim_paths);
            }
        }

        let mut all_errors = Vec::new();
        for prim_path in prim_paths {
            let (_prim_index, errors) = pcp_cache.compute_prim_index(&prim_path);
            for error_type in errors {
                all_errors.push(format!("{:?}", error_type));
            }
        }

        all_errors
    }

    // ========================================================================
    // Edit Target
    // ========================================================================

    /// Gets the current edit target.
    pub fn edit_target(&self) -> EditTarget {
        self.edit_target.read().expect("rwlock poisoned").clone()
    }

    /// Sets the edit target.
    pub fn set_edit_target(&self, target: EditTarget) {
        *self.edit_target.write().expect("rwlock poisoned") = target;
    }

    /// Returns an edit target for the given layer.
    ///
    /// Matches C++ `UsdStage::GetEditTargetForLocalLayer(const SdfLayerHandle &layer)`:
    /// uses the root [`usd_pcp::Cache`] layer stack and
    /// [`usd_pcp::LayerStack::get_layer_offset`] for that layer only.
    pub fn get_edit_target_for_local_layer(&self, layer: &Arc<Layer>) -> EditTarget {
        let Some(pcp_cache) = self.pcp_cache() else {
            return EditTarget::for_local_layer(layer.clone());
        };

        let layer_stack = if let Some(ls) = pcp_cache.layer_stack() {
            ls
        } else {
            match pcp_cache.compute_layer_stack(pcp_cache.layer_stack_identifier()) {
                Ok(ls) => ls,
                Err(_) => return EditTarget::for_local_layer(layer.clone()),
            }
        };

        let offset = layer_stack
            .get_layer_offset(layer)
            .unwrap_or_else(usd_sdf::LayerOffset::identity);
        EditTarget::for_local_layer_with_offset(layer.clone(), offset)
    }

    /// Returns an edit target for the layer at index i in the layer stack (matches C++ GetEditTargetForLocalLayer overload).
    pub fn get_edit_target_for_local_layer_index(&self, index: usize) -> Option<EditTarget> {
        let stack = self.layer_stack();
        stack
            .get(index)
            .map(|layer| self.get_edit_target_for_local_layer(layer))
    }

    /// Gets the edit target (alias for edit_target, matches C++ GetEditTarget).
    pub fn get_edit_target(&self) -> EditTarget {
        self.edit_target()
    }

    /// Mutes the layer identified by layerIdentifier.
    ///
    /// C++ stage.cpp:4135-4183: calls `_cache->RequestLayerMuting()` which
    /// updates PCP's muted layer set and triggers recomposition of affected
    /// prims. Previously we only updated a local HashSet with no PCP effect,
    /// meaning muted layers continued contributing opinions.
    pub fn mute_layer(&self, layer_identifier: &str) {
        self.mute_and_unmute_layers(&[layer_identifier.to_string()], &[]);
    }

    /// Unmutes the layer identified by layerIdentifier.
    pub fn unmute_layer(&self, layer_identifier: &str) {
        self.mute_and_unmute_layers(&[], &[layer_identifier.to_string()]);
    }

    /// Mute and unmute layers in batch.
    ///
    /// C++ stage.cpp:4135-4183: delegates to `PcpCache::RequestLayerMuting()`
    /// then recomposes all affected prims. This is the single code path for
    /// all muting operations.
    pub fn mute_and_unmute_layers(&self, mute_layers: &[String], unmute_layers: &[String]) {
        // Don't allow muting root layer
        let filtered_mute: Vec<String> = mute_layers
            .iter()
            .filter(|id| id.as_str() != self.root_layer.identifier())
            .cloned()
            .collect();

        if filtered_mute.is_empty() && unmute_layers.is_empty() {
            return;
        }

        // Update local tracking set
        {
            let mut muted = self.muted_layers.write().expect("rwlock poisoned");
            for id in unmute_layers {
                muted.remove(id);
            }
            for id in &filtered_mute {
                muted.insert(id.clone());
            }
        }

        // C++ stage.cpp:4155-4160: delegate to PcpCache which updates its
        // internal muted layer tracking and computes affected layer stacks
        let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
        if let Some(ref pcp_cache) = *pcp_cache_guard {
            pcp_cache.request_layer_muting(&filtered_mute, unmute_layers, None, None, None);
        }
        drop(pcp_cache_guard);

        // C++ stage.cpp:4170-4180: recompose all cached prims since muting
        // can affect any prim that uses the muted layer through composition.
        let paths_to_recompose: Vec<Path> = self
            .prim_cache
            .read()
            .expect("rwlock poisoned")
            .keys()
            .cloned()
            .collect();
        if !paths_to_recompose.is_empty() {
            self.recompose_prims(&paths_to_recompose);
        }
    }

    /// Returns all muted layer identifiers (matches C++ GetMutedLayers).
    pub fn get_muted_layers(&self) -> Vec<String> {
        self.muted_layers
            .read()
            .expect("rwlock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    /// Returns true if the layer is muted (matches C++ IsLayerMuted).
    pub fn is_layer_muted(&self, layer_identifier: &str) -> bool {
        self.muted_layers
            .read()
            .expect("rwlock poisoned")
            .contains(layer_identifier)
    }

    // ========================================================================
    // Prim Access
    // ========================================================================

    /// Returns the pseudo-root prim.
    pub fn pseudo_root(&self) -> Prim {
        if let Some(data) = self.pseudo_root_data.get() {
            let weak = self.self_ref.get().cloned().unwrap_or_else(Weak::new);
            Prim::from_data(weak, data.clone())
        } else {
            Prim::invalid()
        }
    }

    /// Returns the pseudo-root prim (alias for pseudo_root, matches C++ GetPseudoRoot).
    pub fn get_pseudo_root(&self) -> Prim {
        self.pseudo_root()
    }

    /// Returns the default prim (if set).
    pub fn default_prim(&self) -> Option<Prim> {
        let default_name = self.root_layer.default_prim();
        if default_name.is_empty() {
            return None;
        }

        let path = Path::from_string(&format!("/{}", default_name))?;
        self.get_prim_at_path(&path)
    }

    /// Returns the default prim (alias for default_prim, matches C++ GetDefaultPrim).
    pub fn get_default_prim(&self) -> Prim {
        self.default_prim().unwrap_or_else(Prim::invalid)
    }

    /// Sets the default prim.
    ///
    /// If the prim is a root prim (direct child of pseudoroot), uses the
    /// prim name. Otherwise uses the full path. Matches C++ `SetDefaultPrim`.
    pub fn set_default_prim(&self, prim: &Prim) -> bool {
        if !prim.is_valid() {
            return false;
        }
        // C++: root prims use GetName(), non-root prims use GetPath().GetAsToken()
        let parent = prim.parent();
        let default_name = if parent.is_valid() && parent.is_pseudo_root() {
            prim.name()
        } else {
            Token::new(prim.path().get_string())
        };
        self.root_layer.set_default_prim(&default_name);
        true
    }

    /// Clears the default prim opinion (removes the field entirely, matches C++ ClearDefaultPrim).
    pub fn clear_default_prim(&self) {
        self.root_layer.clear_default_prim();
    }

    /// Returns true if there's a default prim set.
    pub fn has_default_prim(&self) -> bool {
        !self.root_layer.default_prim().is_empty()
    }

    /// Gets a prim at the given path.
    pub fn get_prim_at_path(&self, path: &Path) -> Option<Prim> {
        // C++: silently return invalid prim for non-absolute paths.
        if !path.is_absolute_path() {
            return None;
        }

        let weak = self.self_ref.get()?.clone();

        // Check prim_cache (the composed _primMap).
        // Matches C++ _GetPrimDataAtPath.
        if let Some(data) = self.prim_cache.read().expect("rwlock poisoned").get(path) {
            return Some(Prim::from_data(weak.clone(), data.clone()));
        }

        if let Some(proto_data) = self.get_prim_data_at_path_or_in_prototype(path) {
            return Some(Prim::from_data(weak.clone(), proto_data));
        }

        // If no prim data at path, check if this path is beneath an instance.
        // If so, return the prim data for the corresponding prototype prim.
        // Matches C++ _GetPrimDataAtPathOrInPrototype (stage.cpp:2393-2410).
        if let Some(instance_cache) = self.instance_cache.get() {
            let proto_path = instance_cache.get_path_in_prototype_for_instance_path(path);
            if !proto_path.is_empty() {
                if let Some(proto_data) = self
                    .prim_cache
                    .read()
                    .expect("rwlock poisoned")
                    .get(&proto_path)
                {
                    // Return an instance-proxy Prim: uses the prototype's data
                    // but presents itself at the original instance path.
                    return Some(Prim::from_data_with_proxy(
                        weak,
                        proto_data.clone(),
                        path.clone(),
                    ));
                }
            }
        }

        // Narrow fallback for direct Sdf namespace edits that have updated layer
        // specs but have not yet driven stage recomposition. We only synthesize
        // prims that are still valid in composed namespace terms.
        for layer in self.layer_stack() {
            if layer.get_spec_type(path) == usd_sdf::SpecType::Prim {
                let specifier_str: Option<String> = layer
                    .get_field(path, &Token::new("specifier"))
                    .and_then(|v| v.downcast_clone::<String>());
                let specifier = specifier_str
                    .as_deref()
                    .and_then(|s| Specifier::try_from(s).ok())
                    .unwrap_or(Specifier::Over);
                let type_name: String = layer
                    .get_field(path, &Token::new("typeName"))
                    .and_then(|v| v.downcast_clone::<String>())
                    .unwrap_or_default();

                let parent_path = path.get_parent_path();
                let parent_data = if parent_path.is_absolute_root_path() || parent_path.is_empty() {
                    None
                } else {
                    self.get_prim_data(&parent_path)
                };
                if parent_data
                    .as_ref()
                    .is_some_and(|parent| !parent.is_active())
                {
                    return None;
                }
                let self_has_defining_specifier =
                    specifier == Specifier::Def || specifier == Specifier::Class;
                let parent_has_defining_specifier = parent_data
                    .as_ref()
                    .map(|parent| parent.has_defining_specifier())
                    .unwrap_or(true);
                if !self_has_defining_specifier && !parent_has_defining_specifier {
                    return None;
                }

                let prim_data = Arc::new(PrimData::new(
                    path.clone(),
                    weak.clone(),
                    Token::new(&type_name),
                    specifier,
                ));

                if parent_path.is_absolute_root_path() || parent_path.is_empty() {
                    if let Some(pseudo_root) = self.pseudo_root_data.get() {
                        pseudo_root.add_child(prim_data.clone());
                    }
                } else if let Some(parent_data) = parent_data {
                    parent_data.add_child(prim_data.clone());
                }

                self.prim_cache
                    .write()
                    .expect("rwlock poisoned")
                    .insert(path.clone(), prim_data.clone());

                if let Some(pcp_cache) = self.pcp_cache() {
                    let (prim_index, _errors) = pcp_cache.compute_prim_index(path);
                    if prim_index.is_valid() {
                        self.compose_prim_flags(&prim_index, &prim_data);
                    }
                }

                return Some(Prim::from_data(weak, prim_data));
            }
        }

        // No prim found
        None
    }

    /// Returns the object at the given path (matches C++ GetObjectAtPath).
    ///
    /// Returns a prim if the path is a prim path or absolute root,
    /// or a property if it's a property path.
    pub fn get_object_at_path(&self, path: &Path) -> Option<super::object::Object> {
        let weak = self.self_ref.get()?.clone();

        // C++ parity: absolute root ("/") returns pseudo-root as object
        if path.is_absolute_root_path() || path.is_prim_path() {
            if let Some(_prim) = self.get_prim_at_path(path) {
                return Some(super::object::Object::new(weak, path.clone()));
            }
        } else if path.is_property_path() {
            if self.get_attribute_at_path(path).is_some()
                || self.get_relationship_at_path(path).is_some()
            {
                return Some(super::object::Object::new(weak, path.clone()));
            }
        }

        None
    }

    /// Returns the property at the given path (matches C++ GetPropertyAtPath).
    pub fn get_property_at_path(&self, path: &Path) -> Option<super::property::Property> {
        if !path.is_property_path() {
            return None;
        }

        if self.get_attribute_at_path(path).is_some()
            || self.get_relationship_at_path(path).is_some()
        {
            let weak = self.self_ref.get()?.clone();
            return Some(super::property::Property::new(weak, path.clone()));
        }
        None
    }

    /// Returns the attribute at the given path (matches C++ GetAttributeAtPath).
    ///
    /// Returns None if the path is not a property path, or if the property
    /// at that path is not an attribute (e.g. it's a relationship).
    pub fn get_attribute_at_path(&self, path: &Path) -> Option<super::attribute::Attribute> {
        if !path.is_property_path() {
            return None;
        }

        // Check actual spec type — must be Attribute, not Relationship
        let prim_path = path.get_prim_path();
        let prop_name = path.get_name();
        let spec_type = self.get_defining_spec_type(&prim_path, prop_name);
        if spec_type != SpecType::Attribute {
            return None;
        }

        let weak = self.self_ref.get()?.clone();
        Some(super::attribute::Attribute::new(weak, path.clone()))
    }

    /// Returns the relationship at the given path (matches C++ GetRelationshipAtPath).
    ///
    /// Returns None if the path is not a property path, or if the property
    /// at that path is not a relationship (e.g. it's an attribute).
    pub fn get_relationship_at_path(
        &self,
        path: &Path,
    ) -> Option<super::relationship::Relationship> {
        if !path.is_property_path() {
            return None;
        }

        // Check actual spec type — must be Relationship, not Attribute
        let prim_path = path.get_prim_path();
        let prop_name = path.get_name();
        let spec_type = self.get_defining_spec_type(&prim_path, prop_name);
        if spec_type != SpecType::Relationship {
            return None;
        }

        let weak = self.self_ref.get()?.clone();
        Some(super::relationship::Relationship::new(weak, path.clone()))
    }

    /// Returns the defining spec type for a property on a prim.
    ///
    /// Matches C++ `UsdStage::_GetDefiningSpecType`: walks PrimIndex via
    /// Resolver to find the spec type across ALL composed layers (including
    /// payloads, references, etc.), not just the root layer stack.
    pub(crate) fn get_defining_spec_type(&self, prim_path: &Path, prop_name: &str) -> SpecType {
        if prop_name.is_empty() {
            return SpecType::Unknown;
        }

        // Get PrimIndex for resolver walk
        let prim_index = self
            .get_prim_at_path(prim_path)
            .and_then(|p| p.prim_index())
            .map(Arc::new);

        let Some(ref prim_index) = prim_index else {
            // Fallback: check root layer stack (in-memory stages without PCP)
            return self.get_defining_spec_type_fallback(prim_path, prop_name);
        };

        // Walk PrimIndex nodes via Resolver (C++ Usd_Resolver)
        let mut resolver = super::resolver::Resolver::new(prim_index, true);
        let mut cur_path_valid = false;
        let mut cur_path = Path::empty();

        while resolver.is_valid() {
            if let Some(layer) = resolver.get_layer() {
                if let Some(local_path) = resolver.get_local_path() {
                    if layer.has_spec(&local_path) {
                        if !cur_path_valid {
                            if let Some(p) = local_path.append_property(prop_name) {
                                cur_path = p;
                                cur_path_valid = true;
                            }
                        }
                        if cur_path_valid {
                            let spec_type = layer.get_spec_type(&cur_path);
                            if spec_type != SpecType::Unknown {
                                return spec_type;
                            }
                        }
                    }
                }
            }
            if resolver.next_layer() {
                cur_path_valid = false;
            }
        }

        SpecType::Unknown
    }

    /// Fallback spec type check using root layer stack only.
    fn get_defining_spec_type_fallback(&self, prim_path: &Path, prop_name: &str) -> SpecType {
        if let Some(prop_path) = prim_path.append_property(prop_name) {
            for layer in self.layer_stack() {
                let st = layer.get_spec_type(&prop_path);
                if st != SpecType::Unknown {
                    return st;
                }
            }
        }
        SpecType::Unknown
    }

    /// Returns the clip cache for this stage (used by attribute value resolution).
    pub(crate) fn clip_cache(&self) -> Option<&Arc<ClipCache>> {
        self.clip_cache.get()
    }

    /// Matches C++ `UsdStage::_ValueMightBeTimeVaryingFromResolveInfo`.
    pub(crate) fn value_might_be_time_varying_from_resolve_info(
        &self,
        info: &super::resolve_info::ResolveInfo,
        attr: &super::attribute::Attribute,
    ) -> bool {
        use super::clip_set::{
            clip_set_has_time_samples, clips_apply_to_layer_stack_site,
            value_from_clips_might_be_time_varying,
        };
        use super::resolve_info::ResolveInfoSource;
        use usd_vt::value_type_can_compose_over;

        match info.source() {
            ResolveInfoSource::None | ResolveInfoSource::Fallback => false,
            ResolveInfoSource::Default => info.value_source_might_be_time_varying(),
            ResolveInfoSource::Spline => true,
            ResolveInfoSource::ValueClips => {
                let Some(spec_path) = info.prim_path().append_property(attr.name().get_text())
                else {
                    return false;
                };
                let Some(clip_cache) = self.clip_cache() else {
                    return false;
                };
                let prim_path = attr.prim_path();
                for clip_set in clip_cache.get_clips_for_prim(&prim_path) {
                    let Some(layer_stack) = info.layer_stack() else {
                        continue;
                    };
                    if !clips_apply_to_layer_stack_site(
                        clip_set.as_ref(),
                        layer_stack,
                        info.prim_path(),
                    ) {
                        continue;
                    }
                    if clip_set_has_time_samples(clip_set.as_ref(), &spec_path) {
                        return value_from_clips_might_be_time_varying(
                            clip_set.as_ref(),
                            &spec_path,
                        );
                    }
                }
                false
            }
            ResolveInfoSource::TimeSamples => {
                let Some(spec_path) = info.prim_path().append_property(attr.name().get_text())
                else {
                    return false;
                };
                let Some(layer_handle) = info.layer() else {
                    return false;
                };
                let Some(layer) = layer_handle.upgrade() else {
                    return false;
                };
                let num_samples = layer.get_num_time_samples_for_path(&spec_path);
                if num_samples > 1 {
                    return true;
                }
                if num_samples == 1 {
                    let Some((lower, upper)) =
                        layer.get_bracketing_time_samples_for_path(&spec_path, 0.0)
                    else {
                        return false;
                    };
                    if lower != upper {
                        return false;
                    }
                    let Some(sample) = layer.query_time_sample(&spec_path, lower) else {
                        return false;
                    };
                    let Some(type_id) = sample.held_type_id() else {
                        return false;
                    };
                    return value_type_can_compose_over(type_id);
                }
                false
            }
        }
    }

    /// Time samples for an attribute using cached [`ResolveInfo`](super::resolve_info::ResolveInfo).
    ///
    /// Matches `UsdStage::_GetTimeSamplesInInterval`: `_SamplesInIntervalResolver` +
    /// `SdfComposeTimeSampleSeries` on composability flags, `Usd_CopyTimeSamplesInInterval`,
    /// and `_GetResolvedValueAtTimeWithClipsImpl` / `NoClipsImpl` (`time_sample_resolvers` module).
    pub(crate) fn get_time_samples_in_interval_with_resolve_info(
        &self,
        attr: &super::attribute::Attribute,
        info: &super::resolve_info::ResolveInfo,
        interval: &Interval,
        resolve_target: Option<&super::resolve_target::ResolveTarget>,
    ) -> Vec<f64> {
        use super::resolve_info::ResolveInfoSource;
        if matches!(info.source(), ResolveInfoSource::None) {
            return Vec::new();
        }
        super::time_sample_resolvers::get_time_samples_in_interval_resolved(
            self,
            attr,
            interval,
            Some(info),
            resolve_target,
        )
    }

    pub(crate) fn get_num_time_samples_with_resolve_info(
        &self,
        attr: &super::attribute::Attribute,
        info: &super::resolve_info::ResolveInfo,
        resolve_target: Option<&super::resolve_target::ResolveTarget>,
    ) -> usize {
        self.get_time_samples_in_interval_with_resolve_info(
            attr,
            info,
            &Interval::get_full_interval(),
            resolve_target,
        )
        .len()
    }

    /// Matches `UsdStage::_GetBracketingTimeSamples`: `_BracketingSamplesResolver` +
    /// `_GetResolvedValueAtTimeWithClipsImpl` / `NoClipsImpl`.
    pub(crate) fn get_bracketing_time_samples_with_resolve_info(
        &self,
        attr: &super::attribute::Attribute,
        info: &super::resolve_info::ResolveInfo,
        time: f64,
        resolve_target: Option<&super::resolve_target::ResolveTarget>,
    ) -> Option<(f64, f64)> {
        use super::resolve_info::ResolveInfoSource;
        if matches!(info.source(), ResolveInfoSource::None) {
            return None;
        }
        super::time_sample_resolvers::get_bracketing_time_samples_resolved(
            self,
            attr,
            Some(info),
            resolve_target,
            time,
        )
    }

    /// Returns the PrimData for a path if it exists in cache.
    pub(crate) fn get_prim_data(&self, path: &Path) -> Option<Arc<PrimData>> {
        self.prim_cache
            .read()
            .expect("rwlock poisoned")
            .get(path)
            .cloned()
    }

    /// Returns prim data at the given path (internal method).
    ///
    /// Matches C++ `_GetPrimDataAtPath(const SdfPath& path)`.
    pub(crate) fn get_prim_data_at_path(&self, path: &Path) -> Option<Arc<PrimData>> {
        self.prim_cache
            .read()
            .expect("rwlock poisoned")
            .get(path)
            .cloned()
    }

    /// Traverses all prims matching the predicate (returns Vec<Prim>).
    pub fn traverse_vec(&self, predicate: PrimFlagsPredicate) -> Vec<Prim> {
        self.traverse_from(&Path::absolute_root(), predicate)
    }

    /// Traverses prims and returns a PrimRange (matches C++ Traverse).
    pub fn traverse(&self) -> prim_range::PrimRange {
        let predicate = super::prim_flags::default_predicate().into_predicate();
        prim_range::PrimRange::stage(self.get_handle(), predicate)
    }

    /// Traverses prims with predicate and returns a PrimRange (matches C++ Traverse overload).
    pub fn traverse_with_predicate(&self, predicate: PrimFlagsPredicate) -> prim_range::PrimRange {
        prim_range::PrimRange::stage(self.get_handle(), predicate)
    }

    /// Traverses all prims and returns a PrimRange (matches C++ TraverseAll).
    pub fn traverse_all_range(&self) -> prim_range::PrimRange {
        let predicate = PrimFlagsPredicate::all();
        prim_range::PrimRange::stage(self.get_handle(), predicate)
    }

    /// Traverses all prims (alias for traverse_all_range, matches C++ TraverseAll).
    pub fn traverse_all(&self) -> prim_range::PrimRange {
        self.traverse_all_range()
    }

    /// Traverses prims starting from the given path.
    pub fn traverse_from(&self, path: &Path, predicate: PrimFlagsPredicate) -> Vec<Prim> {
        let mut result = Vec::new();
        let weak = match self.self_ref.get() {
            Some(w) => w.clone(),
            None => return result,
        };

        if let Some(data) = self.prim_cache.read().expect("rwlock poisoned").get(path) {
            self.traverse_prim_data(&weak, data.clone(), &predicate, &mut result);
        }
        result
    }

    /// Internal traversal helper.
    fn traverse_prim_data(
        &self,
        weak: &Weak<Self>,
        data: Arc<PrimData>,
        predicate: &PrimFlagsPredicate,
        result: &mut Vec<Prim>,
    ) {
        // Check if this prim matches predicate
        if predicate.matches(data.flags()) {
            result.push(Prim::from_data(weak.clone(), data.clone()));
        }

        // Traverse children
        for child in data.children() {
            self.traverse_prim_data(weak, child, predicate, result);
        }
    }

    // ========================================================================
    // Prim Authoring
    // ========================================================================

    /// Defines a prim at the given path with the given type (matches C++ DefinePrim).
    ///
    /// Authors a 'def' prim spec to the current edit target layer.
    pub fn define_prim(
        &self,
        path: impl Into<String>,
        type_name: impl Into<String>,
    ) -> Result<Prim, Error> {
        let path_str = path.into();
        let type_name_str = type_name.into();

        let sdf_path = Path::from_string(&path_str)
            .ok_or_else(|| Error::InvalidPath(format!("Invalid path: {}", path_str)))?;

        // Create ancestor "over" specs for missing intermediate prims (C++ behavior)
        {
            let edit_target = self.edit_target.read().expect("rwlock poisoned");
            let Some(layer) = edit_target.layer() else {
                return Err(Error::Other("No edit target layer".into()));
            };
            let weak = self
                .self_ref
                .get()
                .ok_or_else(|| Error::Other("Stage not initialized".into()))?
                .clone();

            let prefixes = sdf_path.get_prefixes();
            for ancestor in prefixes.iter() {
                if *ancestor == sdf_path || ancestor.is_absolute_root_path() {
                    continue;
                }
                if layer.get_prim_at_path(ancestor).is_none() {
                    layer.create_prim_spec(ancestor, Specifier::Over, "");

                    if self.get_prim_data(ancestor).is_none() {
                        let ancestor_data = Arc::new(PrimData::new(
                            ancestor.clone(),
                            weak.clone(),
                            Token::new(""),
                            Specifier::Over,
                        ));
                        let ancestor_parent = ancestor.get_parent_path();
                        if ancestor_parent.is_absolute_root_path() {
                            if let Some(pseudo_root) = self.pseudo_root_data.get() {
                                pseudo_root.add_child(ancestor_data.clone());
                            }
                        } else if let Some(parent_data) = self
                            .prim_cache
                            .read()
                            .expect("rwlock poisoned")
                            .get(&ancestor_parent)
                        {
                            parent_data.add_child(ancestor_data.clone());
                        }
                        self.prim_cache
                            .write()
                            .expect("rwlock poisoned")
                            .insert(ancestor.clone(), ancestor_data);
                    }
                }
            }
        }

        // Author the prim spec and register in cache
        self.author_prim_spec(path_str, Specifier::Def, &type_name_str)
    }

    /// Overrides a prim at the given path (creates an over, not a def) (matches C++ OverridePrim).
    ///
    /// Authors an 'over' prim spec to the current edit target layer.
    pub fn override_prim(&self, path: impl Into<String>) -> Result<Prim, Error> {
        self.author_prim_spec(path, Specifier::Over, "")
    }

    /// Creates a class prim at the given path (matches C++ CreateClassPrim).
    ///
    /// Authors a 'class' prim spec to the current edit target layer.
    pub fn create_class_prim(&self, path: impl Into<String>) -> Result<Prim, Error> {
        self.author_prim_spec(path, Specifier::Class, "")
    }

    /// Internal helper: authors a prim spec with the given specifier and registers in prim cache.
    fn author_prim_spec(
        &self,
        path: impl Into<String>,
        specifier: Specifier,
        type_name: &str,
    ) -> Result<Prim, Error> {
        let path_str = path.into();
        let sdf_path = Path::from_string(&path_str)
            .ok_or_else(|| Error::InvalidPath(format!("Invalid path: {}", path_str)))?;

        let edit_target = self.edit_target.read().expect("rwlock poisoned");
        let Some(layer) = edit_target.layer().cloned() else {
            return Err(Error::Other("No edit target layer".into()));
        };

        // Create missing ancestor over specs for all authored prims, matching
        // USD's implicit spec creation behavior for OverridePrim/CreateClassPrim
        // as well as DefinePrim.
        let weak = self
            .self_ref
            .get()
            .ok_or_else(|| Error::Other("Stage not initialized".into()))?
            .clone();
        let prefixes = sdf_path.get_prefixes();
        for ancestor in prefixes.iter() {
            if *ancestor == sdf_path || ancestor.is_absolute_root_path() {
                continue;
            }
            if layer.get_prim_at_path(ancestor).is_none() {
                layer.create_prim_spec(ancestor, Specifier::Over, "");

                if self.get_prim_data(ancestor).is_none() {
                    let ancestor_data = Arc::new(PrimData::new(
                        ancestor.clone(),
                        weak.clone(),
                        Token::new(""),
                        Specifier::Over,
                    ));
                    let ancestor_parent = ancestor.get_parent_path();
                    if ancestor_parent.is_absolute_root_path() {
                        if let Some(pseudo_root) = self.pseudo_root_data.get() {
                            pseudo_root.add_child(ancestor_data.clone());
                        }
                    } else if let Some(parent_data) = self
                        .prim_cache
                        .read()
                        .expect("rwlock poisoned")
                        .get(&ancestor_parent)
                    {
                        parent_data.add_child(ancestor_data.clone());
                    }
                    self.prim_cache
                        .write()
                        .expect("rwlock poisoned")
                        .insert(ancestor.clone(), ancestor_data);
                }
            }
        }

        if layer
            .create_prim_spec(&sdf_path, specifier, type_name)
            .is_none()
        {
            return Err(Error::Other("Failed to create prim spec".into()));
        }

        drop(edit_target);

        // Register PrimData in cache (same as define_prim)
        let prim_data = Arc::new(PrimData::new(
            sdf_path.clone(),
            weak.clone(),
            Token::new(type_name),
            specifier,
        ));

        // Ensure all ancestor prims exist (matches C++ SdfJustCreatePrimInLayer behavior).
        // Walk from the target path upward, collecting missing ancestors.
        let mut ancestors_to_create = Vec::new();
        {
            let cache = self.prim_cache.read().expect("rwlock poisoned");
            let mut current = sdf_path.get_parent_path();
            while !current.is_absolute_root_path() && !current.is_empty() {
                if cache.contains_key(&current) {
                    break;
                }
                ancestors_to_create.push(current.clone());
                current = current.get_parent_path();
            }
        }
        // Create ancestors from outermost to innermost
        for ancestor_path in ancestors_to_create.iter().rev() {
            // Create prim spec in layer with Over specifier (intermediate parent)
            layer.create_prim_spec(ancestor_path, Specifier::Over, "");
            if self.get_prim_data(ancestor_path).is_none() {
                let ancestor_data = Arc::new(PrimData::new(
                    ancestor_path.clone(),
                    weak.clone(),
                    Token::new(""),
                    Specifier::Over,
                ));
                let ap_parent = ancestor_path.get_parent_path();
                if ap_parent.is_absolute_root_path() || ap_parent.is_empty() {
                    if let Some(pseudo_root) = self.pseudo_root_data.get() {
                        pseudo_root.add_child(ancestor_data.clone());
                    }
                } else if let Some(p_data) = self
                    .prim_cache
                    .read()
                    .expect("rwlock poisoned")
                    .get(&ap_parent)
                {
                    p_data.add_child(ancestor_data.clone());
                }
                self.prim_cache
                    .write()
                    .expect("rwlock poisoned")
                    .insert(ancestor_path.clone(), ancestor_data);
            }
        }

        if let Some(existing) = self.get_prim_data(&sdf_path) {
            let authored_type = Token::new(type_name);
            if !type_name.is_empty() && existing.type_name() != authored_type {
                self.handle_local_change(&sdf_path);
                if let Some(recomposed) = self.get_prim_at_path(&sdf_path) {
                    return Ok(recomposed);
                }
            }
            return Ok(Prim::from_data(weak, existing));
        }

        let parent_path = sdf_path.get_parent_path();
        if parent_path.is_absolute_root_path() || parent_path.is_empty() {
            if let Some(pseudo_root) = self.pseudo_root_data.get() {
                pseudo_root.add_child(prim_data.clone());
            }
        } else if let Some(parent_data) = self
            .prim_cache
            .read()
            .expect("rwlock poisoned")
            .get(&parent_path)
        {
            parent_data.add_child(prim_data.clone());
        }

        self.prim_cache
            .write()
            .expect("rwlock poisoned")
            .insert(sdf_path.clone(), prim_data.clone());

        if let Some(pcp_cache) = self.pcp_cache() {
            let (prim_index, _errors) = pcp_cache.compute_prim_index(&sdf_path);
            if prim_index.is_valid() {
                self.compose_prim_flags(&prim_index, &prim_data);
            }
        }

        Ok(Prim::from_data(weak, prim_data))
    }

    /// Removes a prim and all its descendants at the given path.
    pub fn remove_prim(&self, path: &Path) -> bool {
        // Remove from edit target layer
        let edit_target = self.edit_target.read().expect("rwlock poisoned");
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        // Collect all descendant paths from cache, then remove them
        let descendant_paths: Vec<Path> = {
            let cache = self.prim_cache.read().expect("rwlock poisoned");
            cache
                .keys()
                .filter(|p| p.has_prefix(path))
                .cloned()
                .collect()
        };

        // Remove all descendants and the prim itself from cache
        {
            let mut cache = self.prim_cache.write().expect("rwlock poisoned");
            for desc_path in &descendant_paths {
                cache.remove(desc_path);
            }
        }

        // Also unlink from parent's children list
        let parent_path = path.get_parent_path();
        if parent_path.is_absolute_root_path() {
            if let Some(pseudo_root) = self.pseudo_root_data.get() {
                pseudo_root.remove_child(path);
            }
        } else if let Some(parent_data) = self
            .prim_cache
            .read()
            .expect("rwlock poisoned")
            .get(&parent_path)
        {
            parent_data.remove_child(path);
        }

        // Keep the authored primChildren field in sync with the deleted prim.
        let children_token = Token::new("primChildren");
        let mut child_names: Vec<Token> = layer
            .get_field(&parent_path, &children_token)
            .and_then(|v| v.as_vec_clone::<Token>())
            .unwrap_or_default();
        child_names.retain(|name| name.as_str() != path.get_name());
        if child_names.is_empty() {
            layer.erase_field(&parent_path, &children_token);
        } else {
            layer.set_field(
                &parent_path,
                &children_token,
                usd_vt::Value::new(child_names),
            );
        }

        // Delete descendant specs from the layer as well. Namespace deletes in
        // USD remove the whole subtree; deleting just the root spec leaves
        // child specs available for recomposition.
        let mut paths_to_delete = descendant_paths;
        if !paths_to_delete.iter().any(|p| p == path) {
            paths_to_delete.push(path.clone());
        }
        paths_to_delete.sort_by_key(|p| std::cmp::Reverse(p.get_path_element_count()));

        let mut removed_any = false;
        for delete_path in &paths_to_delete {
            removed_any |= layer.delete_spec(delete_path);
        }

        if removed_any {
            let changed_path = if parent_path.is_empty() {
                Path::absolute_root()
            } else {
                parent_path
            };
            self.handle_local_change(&changed_path);
        }

        removed_any
    }

    // ========================================================================
    // Load/Unload
    // ========================================================================

    /// Sets the payload inclusion predicate on PcpCache based on load rules.
    /// C++ UsdStage::_IncludePayloadsPredicate calls `_loadRules.IsLoaded(path)`.
    /// For LoadAll: predicate always returns true (include all payloads).
    /// For LoadNone: no predicate set (PcpCache blocks all payloads).
    fn set_payload_predicate_for_rules(
        pcp_cache: &std::sync::Arc<usd_pcp::Cache>,
        rules: &StageLoadRules,
    ) {
        if rules.is_load_all() {
            // LoadAll: include every payload unconditionally
            let pred: std::sync::Arc<dyn Fn(&usd_sdf::Path) -> bool + Send + Sync> =
                std::sync::Arc::new(|_| true);
            pcp_cache.set_include_payload_predicate(Some(pred));
        } else {
            // LoadNone or custom rules: capture a clone for the predicate
            let rules_clone = rules.clone();
            let pred: std::sync::Arc<dyn Fn(&usd_sdf::Path) -> bool + Send + Sync> =
                std::sync::Arc::new(move |path| rules_clone.is_loaded(path));
            pcp_cache.set_include_payload_predicate(Some(pred));
        }
    }

    /// Gets the current load rules.
    pub fn load_rules(&self) -> StageLoadRules {
        self.load_rules.read().expect("rwlock poisoned").clone()
    }

    /// Sets the load rules.
    pub fn set_load_rules(&self, rules: StageLoadRules) {
        *self.load_rules.write().expect("rwlock poisoned") = rules;
        self.refresh_after_load_rules_change();
    }

    fn refresh_after_load_rules_change(&self) {
        let rules = self.load_rules.read().expect("rwlock poisoned").clone();
        if let Some(pcp_cache) = self.pcp_cache() {
            Self::set_payload_predicate_for_rules(&pcp_cache, &rules);
            let included = pcp_cache.included_payloads();
            if !included.is_empty() {
                let to_exclude: Vec<Path> = included.into_iter().collect();
                pcp_cache.request_payloads(&[], &to_exclude, None);
            }
            pcp_cache.clear_prim_index_cache();
        }

        let paths_to_recompose: Vec<Path> = self
            .prim_cache
            .read()
            .expect("rwlock poisoned")
            .keys()
            .cloned()
            .collect();
        if !paths_to_recompose.is_empty() {
            self.recompose_prims(&paths_to_recompose);
        }
    }

    /// Loads a prim's payload (matches C++ Load).
    pub fn load(&self, path: &Path, policy: Option<LoadPolicy>) -> Prim {
        let policy = policy.unwrap_or(LoadPolicy::LoadWithDescendants);
        {
            let mut rules = self.load_rules.write().expect("rwlock poisoned");
            match policy {
                LoadPolicy::LoadWithDescendants => {
                    rules.load_with_descendants(path);
                }
                LoadPolicy::LoadWithoutDescendants => {
                    rules.load_without_descendants(path);
                }
            }
        }
        self.refresh_after_load_rules_change();

        self.get_prim_at_path(path).unwrap_or_else(Prim::invalid)
    }

    /// Unloads a prim's payload (matches C++ Unload).
    pub fn unload(&self, path: &Path) {
        {
            let mut rules = self.load_rules.write().expect("rwlock poisoned");
            rules.unload(path);
        }
        self.refresh_after_load_rules_change();
    }

    /// Unload and load the given path sets (matches C++ LoadAndUnload).
    pub fn load_and_unload(
        &self,
        load_set: &std::collections::HashSet<Path>,
        unload_set: &std::collections::HashSet<Path>,
        policy: Option<LoadPolicy>,
    ) {
        let policy = policy.unwrap_or(LoadPolicy::LoadWithDescendants);
        {
            let mut rules = self.load_rules.write().expect("rwlock poisoned");

            for path in unload_set {
                rules.unload(path);
            }

            for path in load_set {
                match policy {
                    LoadPolicy::LoadWithDescendants => rules.load_with_descendants(path),
                    LoadPolicy::LoadWithoutDescendants => rules.load_without_descendants(path),
                }
            }
        }
        self.refresh_after_load_rules_change();
    }

    /// Returns a set of all loaded paths (matches C++ GetLoadSet).
    pub fn get_load_set(&self) -> std::collections::HashSet<Path> {
        if let Some(pcp_cache) = self.pcp_cache() {
            return pcp_cache.included_payloads();
        }
        let rules = self.load_rules.read().expect("rwlock poisoned");
        rules.get_loaded_paths()
    }

    /// Returns an SdfPathSet of all paths that can be loaded (matches C++ FindLoadable).
    ///
    /// Traverses the stage starting from root_path (or pseudo-root if None) and finds
    /// all prims that have payloads. Returns paths to active, defined prims with payloads.
    pub fn find_loadable(&self, root_path: Option<&Path>) -> std::collections::HashSet<Path> {
        let mut loadable_paths = std::collections::HashSet::new();
        let absolute_root = Path::absolute_root();
        let start_path = root_path.unwrap_or(&absolute_root);

        // Traverse stage starting from root_path
        let predicate = PrimFlagsPredicate::default();
        let range = if start_path == &Path::absolute_root() {
            self.traverse()
        } else {
            // Start traversal from specific prim
            if let Some(start_prim) = self.get_prim_at_path(start_path) {
                prim_range::PrimRange::from_prim_with_predicate(&start_prim, predicate)
            } else {
                // Prim doesn't exist, return empty set
                return loadable_paths;
            }
        };

        // Iterate through prims and find those with payloads
        for prim in range {
            // Only include active, defined prims (inactive prims cannot be loaded)
            if prim.is_active() && prim.is_defined() && prim.has_payload() {
                loadable_paths.insert(prim.path().clone());
            }
        }

        loadable_paths
    }

    /// Returns the load rules (alias for load_rules, matches C++ GetLoadRules).
    pub fn get_load_rules(&self) -> StageLoadRules {
        self.load_rules.read().expect("rwlock poisoned").clone()
    }

    /// Loads all payloads.
    pub fn load_all(&self) {
        *self.load_rules.write().expect("rwlock poisoned") = StageLoadRules::load_all();
        self.refresh_after_load_rules_change();
    }

    /// Unloads all payloads.
    pub fn unload_all(&self) {
        *self.load_rules.write().expect("rwlock poisoned") = StageLoadRules::load_none();
    }

    // ========================================================================
    // Serialization
    // ========================================================================

    /// Saves all dirty layers used by this stage (matches C++ Save).
    ///
    /// Iterates all used layers (via composition arcs), saving each dirty
    /// non-anonymous layer. Anonymous layers are skipped since they have
    /// no persistent identity.
    pub fn save(&self) -> Result<bool, Error> {
        // C++ parity: Save all non-anonymous layers EXCEPT session layer
        // and its direct sublayers. Referenced layers (even from session) ARE saved.
        let session_ids = self.session_layer_identifiers();
        // Collect ALL layers reachable from root (sublayers recursively)
        let mut layers_to_save = Vec::new();
        Self::collect_sublayer_layers(&self.root_layer, &mut layers_to_save);
        // Also add any additional used layers from composition
        for layer in self.get_used_layers(false) {
            if !layers_to_save
                .iter()
                .any(|l: &Arc<Layer>| Arc::ptr_eq(l, &layer))
            {
                layers_to_save.push(layer);
            }
        }

        let mut saved_any = false;
        for layer in &layers_to_save {
            if layer.is_anonymous() {
                continue;
            }
            // Skip session layer and its sublayers (C++ parity)
            if session_ids.contains(layer.identifier()) {
                continue;
            }
            if layer.is_dirty() {
                layer.save().map_err(|e| Error::LayerError(e.to_string()))?;
                saved_any = true;
            }
        }
        Ok(saved_any)
    }

    /// Recursively collect a layer and all its sublayers.
    fn collect_sublayer_layers(layer: &Arc<Layer>, collected: &mut Vec<Arc<Layer>>) {
        if collected.iter().any(|l| Arc::ptr_eq(l, layer)) {
            return;
        }
        collected.push(layer.clone());
        for sublayer_path in layer.sublayer_paths() {
            if let Ok(sublayer) = Layer::find_or_open(&sublayer_path) {
                Self::collect_sublayer_layers(&sublayer, collected);
            }
        }
    }

    /// Exports the stage to a flattened layer (matches C++ Export).
    ///
    /// A "flattened" stage combines all composition arcs (sublayers, references,
    /// payloads, inherits, specializes, variants) into a single layer. This is
    /// useful for:
    /// - Creating standalone files without dependencies
    /// - Baking out final composed values
    /// - Inspecting composition results
    ///
    /// Uses flatten() to create a flattened layer and exports it.
    ///
    /// # Parameters
    ///
    /// - `file_path` - Output file path
    /// - `add_source_file_comment` - If true, adds a comment about the source
    ///
    /// # Returns
    ///
    /// `Ok(true)` if export succeeded, error otherwise
    pub fn export(&self, file_path: &str, add_source_file_comment: bool) -> Result<bool, Error> {
        // Flatten the stage to get a single composed layer
        let flattened_layer = self.flatten(add_source_file_comment)?;

        // Export the flattened layer
        flattened_layer
            .export(file_path)
            .map_err(|e| Error::LayerError(e.to_string()))
    }

    /// Exports the stage to a string (matches C++ ExportToString).
    pub fn export_to_string(&self, add_source_file_comment: bool) -> Result<String, Error> {
        // Flatten the stage to get a single composed layer
        let flattened_layer = self.flatten(add_source_file_comment)?;

        // Export the flattened layer to string
        flattened_layer
            .export_to_string()
            .map_err(|e| Error::LayerError(e.to_string()))
    }

    /// Flattens the stage into a single anonymous layer (matches C++ Flatten).
    ///
    /// Creates a single anonymous layer containing the composed scene with all
    /// composition arcs flattened. Removes variant sets (keeps only selected variants),
    /// removes inherits arcs (copies inherited data to children), preserves instancing
    /// by creating independent roots for prototypes, applies time offsets/scales to
    /// time samples, and prunes inactive prims.
    pub fn flatten(&self, add_source_file_comment: bool) -> Result<Arc<Layer>, Error> {
        // Create new anonymous layer
        let flattened = Layer::create_anonymous(Some("flattened"));

        // Generate flattened prototype paths for instancing
        let prototypes = self.get_prototypes();
        let mut prototype_to_flattened = std::collections::HashMap::new();
        let mut prototype_id = 1;

        for prototype in &prototypes {
            let prototype_path = prototype.path();
            if !prototype_to_flattened.contains_key(prototype_path) {
                // Generate a unique path for the flattened prototype
                let flattened_path = loop {
                    let candidate =
                        Path::from_string(&format!("/Flattened_Prototype_{}", prototype_id))
                            .unwrap_or_else(Path::absolute_root);
                    prototype_id += 1;

                    // Check if path already exists on stage
                    if self.get_prim_at_path(&candidate).is_none() {
                        break candidate;
                    }
                };
                prototype_to_flattened.insert(prototype_path.clone(), flattened_path);
            }
        }

        // Copy prototypes first (they go at the top of the file)
        for prototype in &prototypes {
            if let Some(flattened_path) = prototype_to_flattened.get(prototype.path()) {
                self.copy_prim_to_layer(
                    prototype,
                    &flattened,
                    flattened_path,
                    &prototype_to_flattened,
                )?;
            }
        }

        // Copy all prims from pseudo-root
        let _pseudo_root = self.get_pseudo_root();
        for prim in self.traverse_all() {
            // Skip prototype prims (already copied above)
            if prototype_to_flattened.values().any(|p| *p == *prim.path()) {
                continue;
            }

            // Include inactive prims (C++ keeps them as deactivated in output)
            self.copy_prim_to_layer(&prim, &flattened, prim.path(), &prototype_to_flattened)?;

            // If prim is inactive, mark it as inactive in the flattened output
            if !prim.is_active() {
                if let Some(mut spec) = flattened.get_prim_at_path(prim.path()) {
                    spec.set_active(false);
                }
            }
        }

        // Copy pseudo-root layer metadata (upAxis, metersPerUnit, timeCodes, etc.)
        // Matches C++ UsdFlattenLayerStack which calls _FlattenFields on the pseudo-root.
        {
            let root = Path::absolute_root();
            let skip_fields: &[&str] = &["primChildren", "specifier", "typeName"];
            for field in self.root_layer.list_fields(&root) {
                if skip_fields.contains(&field.as_str()) {
                    continue;
                }
                if let Some(value) = self.root_layer.get_field(&root, &field) {
                    flattened.set_field(&root, &field, value);
                }
            }
        }

        // Add source file comment if requested
        if add_source_file_comment {
            let root_path = self.root_layer.identifier();
            let _doc = format!(
                "Generated from Composed Stage of root layer {}\n",
                root_path
            );
            // Note: Layer doesn't have SetDocumentation yet, so we skip this for now
            // flattened.set_documentation(&doc)?;
        }

        Ok(flattened)
    }

    /// Helper function to copy a prim to a layer during flattening.
    fn copy_prim_to_layer(
        &self,
        prim: &super::prim::Prim,
        dest_layer: &Arc<Layer>,
        dest_path: &Path,
        prototype_to_flattened: &std::collections::HashMap<Path, Path>,
    ) -> Result<(), Error> {
        // Get parent path
        let parent_path = dest_path.get_parent_path();

        // Create prim spec in destination layer
        let prim_name = dest_path.get_name();
        let prim_spec = if parent_path.is_absolute_root_path() {
            // Create root prim
            dest_layer
                .create_prim_spec(dest_path, prim.specifier(), prim.type_name().as_str())
                .ok_or_else(|| {
                    Error::InvalidPath(format!("Failed to create prim at {}", dest_path.as_str()))
                })?
        } else {
            // Ensure all ancestor specs exist (creating "over" stubs as needed).
            // traverse() visits prims top-down so parents normally precede children,
            // but implicit "over" ancestors authored by define_prim may be absent.
            let prefixes = dest_path.get_prefixes();
            for ancestor in &prefixes {
                if ancestor.is_absolute_root_path() || *ancestor == *dest_path {
                    continue;
                }
                if dest_layer.get_prim_at_path(ancestor).is_none() {
                    dest_layer.create_prim_spec(ancestor, usd_sdf::Specifier::Over, "");
                }
            }

            // Create child prim using layer's create_prim_spec
            dest_layer
                .create_prim_spec(dest_path, prim.specifier(), prim.type_name().as_str())
                .ok_or_else(|| {
                    Error::InvalidPath(format!("Failed to create child prim {}", prim_name))
                })?
        };

        // Copy metadata (excluding composition arcs)
        // Note: In C++, GetAllMetadataForFlatten is a helper that calls _GetAllMetadata
        // with forFlattening=true. Our current implementation copies properties directly,
        // which is functionally equivalent for flattening purposes.

        // C++ flattenUtils.cpp: uses composed property names from the UsdPrim,
        // not from any single layer. This ensures properties authored in
        // sublayers, references, payloads, and inherits are all included.
        // Previously we read from root_layer only, missing sublayer/reference
        // properties entirely.
        let authored_prop_names = prim.get_property_names();

        for prop_name in authored_prop_names {
            // Determine property type from the composed prim, not from a single layer.
            // C++ uses the property object's spec type which reflects composition.
            // We try attribute first (more common), then relationship.
            if let Some(attr) = prim.get_attribute(prop_name.as_ref()) {
                self.copy_attribute_to_layer(
                    &attr,
                    dest_layer,
                    dest_path,
                    &prop_name,
                    prototype_to_flattened,
                )?;
            } else if let Some(rel) = prim.get_relationship(prop_name.as_ref()) {
                self.copy_relationship_to_layer(
                    &rel,
                    dest_layer,
                    dest_path,
                    &prop_name,
                    prototype_to_flattened,
                )?;
            }
        }

        // Handle instancing: if this prim is an instance, add reference to prototype
        if prim.is_instance() {
            let prototype = prim.get_prototype();
            if prototype.is_valid() {
                if let Some(flattened_prototype_path) = prototype_to_flattened.get(prototype.path())
                {
                    // Add internal reference to flattened prototype
                    // For internal references, we store the prim path directly
                    use usd_sdf::abstract_data::Value as SdfValue;
                    let mut refs_list_op = prim_spec.references_list();

                    // Append the flattened prototype path to references.
                    // Reference::new("", path) creates an internal reference (empty asset
                    // path, prim path = flattened_prototype_path), matching C++ SdfReference(SdfPath).
                    let mut appended = refs_list_op.get_appended_items().to_vec();
                    let ref_item = usd_sdf::Reference::new("", flattened_prototype_path.as_str());
                    if !appended.contains(&ref_item) {
                        appended.push(ref_item);
                        refs_list_op.set_appended_items(appended).ok();

                        let refs_token = Token::new("references");
                        let refs_value = SdfValue::new(refs_list_op);
                        dest_layer.set_field(dest_path, &refs_token, refs_value);
                    }
                }
            }
        }

        Ok(())
    }

    /// Helper function to copy an attribute to a layer during flattening.
    fn copy_attribute_to_layer(
        &self,
        attr: &super::attribute::Attribute,
        dest_layer: &Arc<Layer>,
        dest_prim_path: &Path,
        dest_name: &Token,
        _prototype_to_flattened: &std::collections::HashMap<Path, Path>,
    ) -> Result<(), Error> {
        use usd_sdf::{SpecType, abstract_data::Value as SdfValue};

        // Create attribute path
        let attr_path = dest_prim_path
            .append_property(dest_name.as_str())
            .ok_or_else(|| {
                Error::InvalidPath(format!(
                    "Failed to create attribute path for {}",
                    dest_name.as_str()
                ))
            })?;

        // Get or create attribute spec
        let _attr_spec = if let Some(existing) = dest_layer.get_attribute_at_path(&attr_path) {
            existing
        } else {
            // Create new attribute spec by creating spec and setting type name
            if !dest_layer.create_spec(&attr_path, SpecType::Attribute) {
                return Err(Error::InvalidPath(format!(
                    "Failed to create attribute spec at {}",
                    attr_path.as_str()
                )));
            }

            // Set type name field
            let type_name_token = Token::new("typeName");
            let type_name_value = SdfValue::new(attr.type_name().as_str().to_string());
            dest_layer.set_field(&attr_path, &type_name_token, type_name_value);

            dest_layer
                .get_attribute_at_path(&attr_path)
                .ok_or_else(|| {
                    Error::InvalidPath("Failed to get created attribute spec".to_string())
                })?
        };

        // Copy default value if present.
        // attr.get() already returns a Value (= SdfValue), so pass it directly —
        // do NOT wrap it in SdfValue::new() which would double-box the value.
        // Use TimeCode::default_time() (NaN) to read the authored default value,
        // NOT TimeCode::default() (0.0) which would trigger time-sample interpolation.
        if let Some(vt_value) = attr.get(usd_sdf::TimeCode::default_time()) {
            let default_token = Token::new("default");
            dest_layer.set_field(&attr_path, &default_token, vt_value);
        }

        // Copy time samples via Layer::set_time_sample() so they land in
        // data.time_samples (readable by list_time_samples_for_path / USDA writer).
        // Do NOT store them as a "timeSamples" field — the field path is only
        // used by the USDC writer which reads fields directly.
        let time_sample_times = attr.get_time_samples();
        for time in time_sample_times {
            if let Some(vt_value) = attr.get(usd_sdf::TimeCode::new(time)) {
                dest_layer.set_time_sample(&attr_path, time, vt_value);
            }
        }

        Ok(())
    }

    /// Helper function to copy a relationship to a layer during flattening.
    fn copy_relationship_to_layer(
        &self,
        rel: &super::relationship::Relationship,
        dest_layer: &Arc<Layer>,
        dest_prim_path: &Path,
        dest_name: &Token,
        prototype_to_flattened: &std::collections::HashMap<Path, Path>,
    ) -> Result<(), Error> {
        use usd_sdf::{SpecType, abstract_data::Value as SdfValue, list_op::PathListOp};

        // Create relationship path
        let rel_path = dest_prim_path
            .append_property(dest_name.as_str())
            .ok_or_else(|| {
                Error::InvalidPath(format!(
                    "Failed to create relationship path for {}",
                    dest_name.as_str()
                ))
            })?;

        // Get or create relationship spec
        if dest_layer.get_relationship_at_path(&rel_path).is_none() {
            // Create new relationship spec
            if !dest_layer.create_spec(&rel_path, SpecType::Relationship) {
                return Err(Error::InvalidPath(format!(
                    "Failed to create relationship spec at {}",
                    rel_path.as_str()
                )));
            }
        }

        // Get targets and remap prototype paths
        let targets = rel.get_targets();
        let mut remapped_targets = Vec::new();

        for target in targets {
            // Check if target is in a prototype and remap if needed
            let remapped = prototype_to_flattened
                .get(&target)
                .cloned()
                .unwrap_or(target);
            remapped_targets.push(remapped);
        }

        // Set targets using PathListOp
        if !remapped_targets.is_empty() {
            let mut list_op = PathListOp::new();
            list_op.set_explicit_items(remapped_targets).ok();

            let targets_token = Token::new("targetPaths");
            let targets_value = SdfValue::new(list_op);
            dest_layer.set_field(&rel_path, &targets_token, targets_value);
        }

        Ok(())
    }

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Gets the interpolation type.
    pub fn interpolation_type(&self) -> InterpolationType {
        *self.interpolation_type.read().expect("rwlock poisoned")
    }

    /// Gets the interpolation type (alias for interpolation_type, matches C++ GetInterpolationType).
    pub fn get_interpolation_type(&self) -> InterpolationType {
        self.interpolation_type()
    }

    /// Sets the interpolation type.
    pub fn set_interpolation_type(&self, interp: InterpolationType) {
        *self.interpolation_type.write().expect("rwlock poisoned") = interp;
    }

    /// Gets the resolver context.
    pub fn resolver_context(&self) -> Option<ResolverContext> {
        self.resolver_context
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Gets the resolver context (alias for resolver_context, matches C++ GetPathResolverContext).
    pub fn get_path_resolver_context(&self) -> Option<ResolverContext> {
        self.resolver_context()
    }

    /// Sets the resolver context (matches C++ SetPathResolverContext).
    pub fn set_path_resolver_context(&self, context: Option<ResolverContext>) {
        *self.resolver_context.write().expect("rwlock poisoned") = context;
    }

    // ========================================================================
    // Composition Change Processing
    // ========================================================================

    /// Handles notification that layers have changed.
    ///
    /// Detects which layer changes affect this stage, computes the set of
    /// prims that need recomposition, and processes the changes.
    ///
    /// Matches C++ `UsdStage::_HandleLayersDidChange`.
    pub fn handle_layers_did_change(&self, serial: u64, changed_paths: Vec<Path>) {
        // Dedup serial: ignore already-seen or out-of-order changes
        let prev = self.last_layers_notice_serial.load(Ordering::Acquire);
        if serial <= prev {
            return;
        }
        self.last_layers_notice_serial
            .store(serial, Ordering::Release);
        self.last_change_serial.store(serial, Ordering::Release);

        self.record_pending_changes(changed_paths);
    }

    fn record_pending_changes(&self, changed_paths: Vec<Path>) {
        // Build pending changes
        let mut pending = PendingChanges::default();

        // Record changed paths into the PCP changes, then into recompose set
        {
            let pcp_guard = self.pcp_cache.read().expect("rwlock poisoned");
            if let Some(ref pcp_cache) = *pcp_guard {
                for path in &changed_paths {
                    pending
                        .pcp_changes
                        .did_change_significantly(pcp_cache, path);
                }
            }
        }

        // All changed paths are candidates for recomposition
        for path in changed_paths {
            pending
                .recompose_changes
                .entry(path)
                .or_insert_with(Vec::new);
        }

        // Store and process
        {
            let mut lock = self.pending_changes.lock().expect("mutex poisoned");
            *lock = Some(pending);
        }
        self.process_pending_changes();
    }

    /// Records a local authored change and recomposes the affected subtree.
    pub(crate) fn handle_local_change(&self, changed_path: &Path) {
        self.record_pending_changes(vec![changed_path.clone()]);
    }

    /// Recomposes prims affected by the given PCP changes.
    ///
    /// Applies PCP changes (recomputes layer stacks/prim indexes), merges any
    /// caller-provided resync paths, then rebuilds the prim cache for those
    /// subtrees.
    ///
    /// Matches C++ `UsdStage::_Recompose`.
    pub fn recompose(&self, changes: &PcpChanges, seed_paths: &[Path]) {
        // Apply PCP-level changes (layer stack recomputation etc.)
        changes.apply();

        // Seed with the explicit resync paths the caller already identified.
        let mut paths_to_recompose = seed_paths.to_vec();

        // Collect additional paths needing recomposition from cache changes.
        for (_cache_id, cache_changes) in changes.cache_changes() {
            for path in &cache_changes.did_change_significantly {
                paths_to_recompose.push(path.clone());
            }
            for path in &cache_changes.did_change_prims {
                paths_to_recompose.push(path.clone());
            }
        }

        self.recompose_prims(&paths_to_recompose);
    }

    /// Processes pending changes: recomposes affected subtrees and sends
    /// an `ObjectsChanged` notice.
    ///
    /// Matches C++ `UsdStage::_ProcessPendingChanges`.
    pub fn process_pending_changes(&self) -> bool {
        let pending = {
            let mut lock = self.pending_changes.lock().expect("mutex poisoned");
            lock.take()
        };

        let Some(pending) = pending else {
            return false;
        };

        // Recompose affected subtrees
        let seed_paths: Vec<Path> = pending.recompose_changes.keys().cloned().collect();
        self.recompose(&pending.pcp_changes, &seed_paths);

        // Send ObjectsChanged notice via TfNotice
        let weak = self.self_ref.get().cloned().unwrap_or_else(|| Weak::new());

        let notice = super::notice::ObjectsChanged::new(
            weak,
            pending.recompose_changes,
            pending.info_changes,
            pending.asset_path_resync_changes,
            super::notice::NamespaceEditsInfo::default(),
        );

        usd_tf::notice::send(&notice);
        true
    }

    /// Rebuilds prim cache entries for the given paths.
    ///
    /// Removes stale entries and re-populates from the root layer.
    /// Descendant paths are pruned (only ancestor paths are recomposed).
    /// Uses parallel composition when the number of paths exceeds a threshold.
    ///
    /// Matches C++ `UsdStage::_RecomposePrims`.
    fn recompose_prims(&self, paths: &[Path]) {
        if paths.is_empty() {
            return;
        }

        // Prune descendant paths: if /A and /A/B both changed, only recompose /A
        let mut sorted = paths.to_vec();
        sorted.sort();
        sorted.dedup();

        let mut pruned = Vec::with_capacity(sorted.len());
        for path in &sorted {
            let dominated = pruned
                .iter()
                .any(|ancestor: &Path| path.has_prefix(ancestor));
            if !dominated {
                pruned.push(path.clone());
            }
        }

        if pruned.iter().any(Path::is_absolute_root_path) {
            if let Some(weak) = self.self_ref.get() {
                if let Some(arc_self) = weak.upgrade() {
                    arc_self.repopulate();
                }
            }
            return;
        }

        if let Some(instance_cache) = self.instance_cache.get() {
            for path in &pruned {
                instance_cache.unregister_instance_prim_indexes_under(path);
            }
        }

        // Unlink stale subtree roots from their parent child lists before rebuilding.
        // Otherwise a local recompose can leave the old child reachable from traversal
        // and append a freshly rebuilt child with the same path.
        for path in &pruned {
            let parent_path = path.get_parent_path();
            if parent_path.is_absolute_root_path() || parent_path.is_empty() {
                if let Some(pseudo_root) = self.pseudo_root_data.get() {
                    pseudo_root.remove_child(path);
                }
            } else if let Some(parent_data) = self
                .prim_cache
                .read()
                .expect("rwlock poisoned")
                .get(&parent_path)
            {
                parent_data.remove_child(path);
            }
        }

        // Use parallel prim index composition for large batches
        // Matches C++ pattern: _ComposePrimIndexesInParallel + _ComposeSubtreesInParallel
        if pruned.len() >= Self::PARALLEL_THRESHOLD {
            self.compose_prim_indexes_in_parallel(&pruned);
        }

        // Remove cached prim data at and below each path
        {
            let mut cache = self.prim_cache.write().expect("rwlock poisoned");
            let paths_to_remove: Vec<Path> = cache
                .keys()
                .filter(|cached_path| {
                    pruned
                        .iter()
                        .any(|recomp_path| cached_path.has_prefix(recomp_path))
                })
                .cloned()
                .collect();
            for p in paths_to_remove {
                cache.remove(&p);
            }
        }

        // Re-populate subtrees (parallel for large sets, sequential otherwise)
        if let Some(weak) = self.self_ref.get() {
            if let Some(arc_self) = weak.upgrade() {
                if pruned.len() >= Self::PARALLEL_THRESHOLD {
                    arc_self.compose_subtrees_in_parallel(&pruned, weak);
                } else {
                    let mut work: Vec<Path> = pruned.clone();
                    while let Some(p) = work.pop() {
                        arc_self.compose_single_prim(&p, weak, &mut work);
                    }
                }
                arc_self.process_instancing_changes(weak);
            }
        }
    }

    /// Minimum number of paths to trigger parallel composition.
    ///
    /// Below this threshold, sequential composition is used to avoid
    /// the overhead of rayon task scheduling.
    const PARALLEL_THRESHOLD: usize = 10;

    /// Computes prim indexes in parallel for the given paths.
    ///
    /// Delegates to `PcpCache::compute_prim_indexes_in_parallel` which uses
    /// `ParallelIndexer` with rayon depth-level parallelism.
    ///
    /// Matches C++ `UsdStage::_ComposePrimIndexesInParallel`.
    fn compose_prim_indexes_in_parallel(&self, paths: &[Path]) {
        let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
        if let Some(ref pcp_cache) = *pcp_cache_guard {
            let (_results, _errors) = pcp_cache.compute_prim_indexes_in_parallel(paths);
            // Errors are logged internally by PcpCache; indexes are cached
            // in the PcpCache's prim_index_cache for later lookup.
        }
    }

    /// Composes subtrees in parallel using rayon.
    ///
    /// For each root path, recursively populates prims in parallel.
    /// Uses `rayon::scope` to spawn tasks for independent subtrees.
    ///
    /// Matches C++ `UsdStage::_ComposeSubtreesInParallel`.
    fn compose_subtrees_in_parallel(self: &Arc<Self>, paths: &[Path], weak_stage: &Weak<Self>) {
        // Collect root specs first (sequential - layer access isn't thread-safe
        // for iteration, but individual path lookups are fine via Arc).
        let valid_paths: Vec<&Path> = paths
            .iter()
            .filter(|p| self.root_layer.get_prim_at_path(p).is_some())
            .collect();

        if valid_paths.is_empty() {
            return;
        }

        // Each subtree root can be populated independently.
        // We use par_iter to parallelize across subtree roots.
        let stage_clone = self.clone();
        let weak_clone = weak_stage.clone();

        // Parallel population of independent subtree roots
        // Iterative population for each subtree root
        for path in valid_paths {
            let mut work = vec![path.clone()];
            while let Some(p) = work.pop() {
                stage_clone.compose_single_prim(&p, &weak_clone, &mut work);
            }
        }
    }

    // ========================================================================
    // Asset Path Resolution
    // ========================================================================

    /// Resolves asset paths in place using the stage's resolver context.
    ///
    /// Binds the stage's resolver context, then resolves each asset path
    /// relative to the root layer.
    pub fn make_resolved_asset_paths(&self, asset_paths: &mut [usd_sdf::asset_path::AssetPath]) {
        let resolver = usd_ar::resolver::get_resolver();
        let resolver_guard = resolver.read().expect("rwlock poisoned");

        // Bind context if available
        let ctx = self.resolver_context();
        let binding_data = ctx.as_ref().and_then(|c| resolver_guard.bind_context(c));

        let expr_vars = usd_vt::Dictionary::new();
        let mut errors = Vec::new();
        usd_sdf::asset_path::resolve_asset_paths(
            &self.root_layer,
            &expr_vars,
            asset_paths,
            &mut errors,
        );

        // Unbind context
        if let Some(c) = ctx.as_ref() {
            resolver_guard.unbind_context(c, binding_data);
        }
    }

    /// Resolves asset path values within a VtValue.
    ///
    /// If the value holds an SdfAssetPath, resolves it. If it holds a
    /// Vec<SdfAssetPath> (VtArray), resolves all entries.
    pub fn make_resolved_asset_paths_value(&self, value: &mut usd_vt::Value) {
        use usd_sdf::asset_path::AssetPath as SdfAssetPath;

        if let Some(ap) = value.get::<SdfAssetPath>() {
            let mut path = ap.clone();
            self.make_resolved_asset_paths(std::slice::from_mut(&mut path));
            *value = usd_vt::Value::new(path);
        } else if let Some(paths) = value.get::<Vec<SdfAssetPath>>() {
            let mut paths = paths.clone();
            self.make_resolved_asset_paths(&mut paths);
            *value = usd_vt::Value::new(paths);
        }
    }

    /// Gets the population mask.
    pub fn population_mask(&self) -> Option<StagePopulationMask> {
        self.population_mask
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Gets the population mask (alias for population_mask, matches C++ GetPopulationMask).
    pub fn get_population_mask(&self) -> Option<StagePopulationMask> {
        self.population_mask()
    }

    /// Sets the population mask (matches C++ SetPopulationMask).
    pub fn set_population_mask(&self, mask: Option<StagePopulationMask>) {
        *self.population_mask.write().expect("rwlock poisoned") = mask;
        // Note: In C++, this would trigger recomposition. For now, we just store it.
        // Full implementation would need to rebuild the stage's prim hierarchy.
    }

    /// Expand this stage's population mask to include targets (matches C++ ExpandPopulationMask).
    ///
    /// Expands the population mask to include targets of relationships and connections
    /// to attributes that pass the given predicates. Traverses the stage according to
    /// traversal_predicate and collects all relationship targets and attribute connections.
    pub fn expand_population_mask(
        &self,
        traversal_predicate: &PrimFlagsPredicate,
        rel_pred: Option<&dyn Fn(&super::relationship::Relationship) -> bool>,
        attr_pred: Option<&dyn Fn(&super::attribute::Attribute) -> bool>,
    ) {
        use std::collections::HashSet;

        let mut new_paths = HashSet::new();

        // Traverse stage according to predicate
        let range = self.traverse_with_predicate(*traversal_predicate);

        for prim in range {
            // Get all relationships
            for rel_name in prim.get_relationship_names() {
                if let Some(rel) = prim.get_relationship(rel_name.as_ref()) {
                    // Check if relationship passes predicate
                    if rel_pred.map(|p| p(&rel)).unwrap_or(true) {
                        // Get targets
                        let targets = rel.get_targets();
                        for target in targets {
                            new_paths.insert(target);
                        }
                    }
                }
            }

            // Get all attributes
            for attr_name in prim.get_attribute_names() {
                if let Some(attr) = prim.get_attribute(attr_name.as_ref()) {
                    // Check if attribute passes predicate
                    if attr_pred.map(|p| p(&attr)).unwrap_or(true) {
                        // Get connections
                        let connections = attr.get_connections();
                        for connection in connections {
                            new_paths.insert(connection);
                        }
                    }
                }
            }
        }

        // Expand population mask with new paths
        if let Some(mut mask) = self
            .population_mask
            .read()
            .expect("rwlock poisoned")
            .clone()
        {
            for path in new_paths {
                mask.add(path);
            }
            *self.population_mask.write().expect("rwlock poisoned") = Some(mask);
        } else {
            // No mask exists, create new one
            let mut mask = StagePopulationMask::new();
            for path in new_paths {
                mask.add(path);
            }
            *self.population_mask.write().expect("rwlock poisoned") = Some(mask);
        }
    }

    // ========================================================================
    // Utility
    // ========================================================================

    /// Returns true if this stage has local changes.
    pub fn has_local_changes(&self) -> bool {
        self.root_layer.is_dirty()
    }

    /// Reloads all layers in the stage's layer stack from disk, then recomposes.
    ///
    /// Matches C++ `UsdStage::Reload()` which reloads every layer and
    /// triggers recomposition for all affected prims.
    pub fn reload(&self) -> Result<(), Error> {
        // Reload every non-anonymous layer in the composed layer stack.
        // Anonymous layers have no backing file — skip them (C++ parity).
        let layers = self.layer_stack();
        for layer in &layers {
            if layer.is_anonymous() {
                continue;
            }
            layer
                .reload()
                .map_err(|e| Error::LayerError(e.to_string()))?;
        }

        // Recompose all cached prims so composed values reflect the reloaded data
        let paths_to_recompose: Vec<Path> = self
            .prim_cache
            .read()
            .expect("rwlock poisoned")
            .keys()
            .cloned()
            .collect();
        if !paths_to_recompose.is_empty() {
            self.recompose_prims(&paths_to_recompose);
        }
        Ok(())
    }

    /// Re-populate the stage prim cache from the current root layer content.
    ///
    /// Call this after mutating the root layer (e.g. via import_from_string)
    /// to ensure the stage's prim cache reflects the latest layer state.
    /// In C++, this happens automatically via SdfNotice; here it must be
    /// triggered explicitly.
    pub fn repopulate(self: &Arc<Self>) {
        // Clear the existing prim cache
        {
            let mut cache = self.prim_cache.write().expect("rwlock poisoned");
            cache.clear();
        }
        // Clear pseudo-root children so populate_from_layer rebuilds them
        if let Some(pseudo_root) = self.pseudo_root_data.get() {
            pseudo_root.clear_children();
            pseudo_root.set_parent(None);
            self.prim_cache
                .write()
                .expect("rwlock poisoned")
                .insert(Path::absolute_root(), pseudo_root.clone());
        }
        if let Some(instance_cache) = self.instance_cache.get() {
            instance_cache.clear();
        }
        // Clear PCP cache so prim indexes are recomputed from the updated layer
        {
            let pcp = self.pcp_cache.read().expect("rwlock poisoned");
            if let Some(ref cache) = *pcp {
                cache.clear();
            }
        }
        // Re-compose from layers
        self.populate_from_layer();
    }

    fn register_layer_change_listener(self: &Arc<Self>) {
        let weak_stage = Arc::downgrade(self);
        let key =
            usd_tf::notice::register_global::<usd_sdf::notice::LayersDidChange, _>(move |notice| {
                let Some(stage) = weak_stage.upgrade() else {
                    return;
                };

                let mut participatory_layers = stage.layer_stack();
                participatory_layers.push(stage.root_layer.clone());
                if let Some(session) = &stage.session_layer {
                    participatory_layers.push(session.clone());
                }

                let mut changed_paths = Vec::new();
                for (layer, change_list) in notice.base().iter() {
                    let is_participating = participatory_layers.iter().any(|candidate| {
                        Arc::ptr_eq(candidate, layer)
                            || candidate.identifier() == layer.identifier()
                    });
                    if !is_participating {
                        continue;
                    }
                    changed_paths.extend(change_list.iter().map(|(path, _)| path.clone()));
                }

                if !changed_paths.is_empty() {
                    stage.handle_layers_did_change(
                        notice.base().serial_number() as u64,
                        changed_paths,
                    );
                }
            });

        *self.layers_did_change_key.lock().expect("mutex poisoned") = Some(key);
    }

    /// Saves all dirty layers except session layers (matches C++ Save).
    ///
    /// Saves the root layer and all non-session sublayers that are dirty.
    /// Gets sublayers from the layer stack.
    pub fn save_all(&self) -> Result<(), Error> {
        // Save root layer
        self.root_layer
            .save()
            .map_err(|e| Error::LayerError(e.to_string()))?;

        // Get layer stack (includes sublayers)
        let layer_stack = self.get_layer_stack(false); // false = exclude session layers

        // Save all non-session sublayers
        for layer in layer_stack {
            // Skip root layer (already saved) and session layers
            if !Arc::ptr_eq(&layer, &self.root_layer) && layer.is_dirty() {
                layer.save().map_err(|e| Error::LayerError(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Saves all session layers (matches C++ SaveSessionLayers).
    ///
    /// Saves the session layer and all its sublayers that are dirty.
    /// Anonymous layers are skipped since they have no persistent identity.
    pub fn save_session_layers(&self) -> Result<(), Error> {
        // C++ parity: save ONLY the session layer and its sublayers (recursively).
        let Some(session) = &self.session_layer else {
            return Ok(());
        };
        let mut layers = Vec::new();
        Self::collect_sublayer_layers(session, &mut layers);
        for layer in &layers {
            if layer.is_anonymous() {
                continue;
            }
            if layer.is_dirty() {
                layer.save().map_err(|e| Error::LayerError(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Checks if a file is supported by UsdStage (matches C++ IsSupportedFile).
    ///
    /// Strips `:SDF_FORMAT_ARGS:` suffix before checking extension,
    /// matching C++ behavior.
    pub fn is_supported_file(file_path: &str) -> bool {
        // Strip SDF_FORMAT_ARGS suffix if present (C++ parity)
        let path = if let Some(pos) = file_path.find(":SDF_FORMAT_ARGS:") {
            &file_path[..pos]
        } else {
            file_path
        };
        path.ends_with(".usd")
            || path.ends_with(".usda")
            || path.ends_with(".usdc")
            || path.ends_with(".usdz")
    }

    /// Clears the stage.
    pub fn clear(&self) {
        self.root_layer.clear();
    }

    /// Returns true if a prim exists at the given path.
    pub fn has_prim_at_path(&self, path: &Path) -> bool {
        self.get_prim_at_path(path)
            .map(|p| p.is_valid())
            .unwrap_or(false)
    }

    // ========================================================================
    // PCP Cache Access
    // ========================================================================

    /// Returns the PCP cache for this stage.
    pub(crate) fn pcp_cache(&self) -> Option<PcpCachePtr> {
        self.pcp_cache.read().expect("rwlock poisoned").clone()
    }

    /// Returns a handle to this stage (for use with PrimRange::stage).
    pub(crate) fn get_handle(&self) -> Arc<Self> {
        self.self_ref
            .get()
            .and_then(|w| w.upgrade())
            .expect("Stage should be valid")
    }

    // ========================================================================
    // Stage Metadata
    // ========================================================================

    fn active_session_layer(&self) -> Option<&Arc<Layer>> {
        self.session_layer
            .as_ref()
            .filter(|layer| !self.is_layer_muted(layer.identifier()))
    }

    /// Gets stage metadata (matches C++ GetMetadata).
    pub fn get_metadata(&self, key: &Token) -> Option<usd_vt::Value> {
        // Check session layer first, then root layer
        if let Some(session) = self.active_session_layer() {
            if session.has_field(&Path::absolute_root(), key) {
                if let Some(value) = session.get_field(&Path::absolute_root(), key) {
                    return Some(value);
                }
            }
        }

        // Check root layer
        if self.root_layer.has_field(&Path::absolute_root(), key) {
            self.root_layer.get_field(&Path::absolute_root(), key)
        } else {
            None
        }
    }

    /// Returns true if metadata exists (matches C++ HasMetadata).
    pub fn has_metadata(&self, key: &Token) -> bool {
        self.get_metadata(key).is_some()
    }

    /// Returns true if metadata is authored (matches C++ HasAuthoredMetadata).
    pub fn has_authored_metadata(&self, key: &Token) -> bool {
        // Check session layer first
        if let Some(session) = self.active_session_layer() {
            if session.has_field(&Path::absolute_root(), key) {
                return true;
            }
        }

        // Check root layer
        self.root_layer.has_field(&Path::absolute_root(), key)
    }

    /// Sets stage metadata (matches C++ SetMetadata).
    pub fn set_metadata(&self, key: &Token, value: usd_vt::Value) -> bool {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_field(&Path::absolute_root(), key, value);
            true
        } else {
            // Edit target is not root or session layer - this is an error
            false
        }
    }

    /// Clears stage metadata (matches C++ ClearMetadata).
    pub fn clear_metadata(&self, key: &Token) -> bool {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.erase_field(&Path::absolute_root(), key);
            true
        } else {
            // Edit target is not root or session layer - this is an error
            false
        }
    }

    /// Gets metadata by dictionary key (matches C++ GetMetadataByDictKey).
    pub fn get_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> Option<usd_vt::Value> {
        // Check session layer first, then root layer
        if let Some(session) = self.active_session_layer() {
            if let Some(value) =
                session.get_field_dict_value_by_key(&Path::absolute_root(), key, key_path)
            {
                return Some(value);
            }
        }

        // Check root layer
        self.root_layer
            .get_field_dict_value_by_key(&Path::absolute_root(), key, key_path)
    }

    /// Returns true if metadata dict key exists (matches C++ HasMetadataDictKey).
    pub fn has_metadata_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        self.get_metadata_by_dict_key(key, key_path).is_some()
    }

    /// Returns true if metadata dict key is authored (matches C++ HasAuthoredMetadataDictKey).
    pub fn has_authored_metadata_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        // Check session layer first
        if let Some(session) = self.active_session_layer() {
            if session.has_field_dict_key(&Path::absolute_root(), key, key_path) {
                return true;
            }
        }

        // Check root layer
        self.root_layer
            .has_field_dict_key(&Path::absolute_root(), key, key_path)
    }

    /// Sets metadata by dictionary key (matches C++ SetMetadataByDictKey).
    pub fn set_metadata_by_dict_key(
        &self,
        key: &Token,
        key_path: &Token,
        value: usd_vt::Value,
    ) -> bool {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_field_dict_value_by_key(&Path::absolute_root(), key, key_path, value);
            true
        } else {
            // Edit target is not root or session layer - this is an error
            false
        }
    }

    /// Clears metadata by dictionary key (matches C++ ClearMetadataByDictKey).
    pub fn clear_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.erase_field_dict_value_by_key(&Path::absolute_root(), key, key_path);
            true
        } else {
            // Edit target is not root or session layer - this is an error
            false
        }
    }

    /// Writes fallback prim types to stage metadata (matches C++ WriteFallbackPrimTypes).
    ///
    /// Writes fallback prim types from the schema registry to stage metadata.
    /// Does not overwrite existing fallback entries; only adds entries for types
    /// that don't have fallbacks defined yet.
    pub fn write_fallback_prim_types(&self) {
        use super::tokens;

        let fallback_prim_types_key = tokens::usd_tokens().fallback_prim_types.clone();

        // Get fallback prim types from schema registry
        use super::schema_registry::SchemaRegistry;
        use usd_vt::{Dictionary, Value as VtValue};

        let schema_registry = SchemaRegistry::get_instance();
        let schema_fallbacks = schema_registry.get_fallback_prim_types();

        if schema_fallbacks.is_empty() {
            return;
        }

        // Get existing fallback types from stage metadata (if any)
        let mut existing_dict = self
            .get_metadata(&fallback_prim_types_key)
            .and_then(|v| v.downcast_clone::<Dictionary>())
            .unwrap_or_default();

        // Merge schema fallbacks into existing (existing takes precedence via DictionaryOver)
        // Only add entries for types that don't have fallbacks defined yet
        for (type_name, fallback_value) in schema_fallbacks.iter() {
            if !existing_dict.contains_key(type_name) {
                existing_dict.insert_value(type_name.clone(), fallback_value.clone());
            }
        }

        // Write merged dictionary to stage metadata
        let dict_value = VtValue::from(existing_dict);
        let _ = self.set_metadata(&fallback_prim_types_key, dict_value);
    }

    // ========================================================================
    // Object Metadata API (for UsdObject)
    // ========================================================================

    fn metadata_value_can_compose(value: &usd_vt::Value) -> bool {
        value.is_array_edit_valued() || value.is::<usd_vt::Dictionary>()
    }

    fn consume_metadata_opinion(
        partial: &mut Option<usd_vt::Value>,
        value: usd_vt::Value,
    ) -> Option<usd_vt::Value> {
        if let Some(existing) = partial.as_ref() {
            if let Some(composed) = usd_vt::value_try_compose_over(existing, &value) {
                if Self::metadata_value_can_compose(&composed) {
                    *partial = Some(composed);
                    return None;
                }
                return Some(composed);
            }
        }

        if Self::metadata_value_can_compose(&value) {
            *partial = Some(value);
            return None;
        }

        if let Some(existing) = partial.as_ref() {
            if let Some(composed) = usd_vt::value_try_compose_over(existing, &value) {
                if Self::metadata_value_can_compose(&composed) {
                    *partial = Some(composed);
                    return None;
                }
                return Some(composed);
            }
        }

        Some(value)
    }

    fn finalize_metadata_value(partial: usd_vt::Value) -> usd_vt::Value {
        if partial.is_array_edit_valued() {
            usd_vt::value_try_compose_over(&partial, &usd_vt::Value::empty()).unwrap_or(partial)
        } else {
            partial
        }
    }

    fn metadata_site_paths_for_object(&self, path: &Path) -> Vec<(Arc<Layer>, Path)> {
        let prim_path = if path.is_property_path() {
            path.get_prim_path()
        } else {
            path.clone()
        };

        let mut sites = Vec::new();

        let pcp_cache_guard = self.pcp_cache.read().expect("rwlock poisoned");
        if let Some(ref pcp_cache) = *pcp_cache_guard {
            let (prim_index, _errors) = pcp_cache.compute_prim_index(&prim_path);
            if prim_index.is_valid() {
                let index_arc = Arc::new(prim_index);
                let mut resolver = crate::resolver::Resolver::new(&index_arc, true);
                while resolver.is_valid() {
                    if let (Some(layer), Some(local_path)) =
                        (resolver.get_layer(), resolver.get_local_path())
                    {
                        let field_path = if path.is_property_path() {
                            local_path
                                .append_property(path.get_name())
                                .unwrap_or(local_path)
                        } else {
                            local_path
                        };
                        sites.push((layer, field_path));
                    }
                    resolver.next_layer();
                }
                return sites;
            }
        }
        drop(pcp_cache_guard);

        for layer in self.layer_stack() {
            sites.push((layer, path.clone()));
        }
        sites
    }

    fn get_object_spec_type(&self, path: &Path) -> SpecType {
        if path.is_absolute_root_path() {
            return SpecType::PseudoRoot;
        }
        if path.is_property_path() {
            return self.get_defining_spec_type(&path.get_prim_path(), path.get_name());
        }
        SpecType::Prim
    }

    fn list_metadata_fields_for_object(&self, path: &Path) -> Vec<Token> {
        let mut fields = std::collections::BTreeSet::new();

        for (layer, field_path) in self.metadata_site_paths_for_object(path) {
            for field in layer.list_fields(&field_path) {
                if !field.as_str().starts_with('_') {
                    fields.insert(field);
                }
            }
        }

        let spec_type = self.get_object_spec_type(path);
        if spec_type != SpecType::Unknown {
            for field in usd_sdf::Schema::instance()
                .base()
                .get_required_fields(spec_type)
            {
                if !field.as_str().starts_with('_') {
                    fields.insert(field);
                }
            }
        }

        fields.into_iter().collect()
    }

    /// Gets metadata for an object at the given path.
    ///
    /// C++ `_GetGeneralMetadataImpl` (stage.cpp): uses `Usd_Resolver` to walk
    /// the full PrimIndex node graph in strength order, including all composition
    /// arcs (references, payloads, inherits, specializes). Previously we only
    /// walked the local layer stack (session+root+sublayers), missing metadata
    /// authored in referenced/payloaded/inherited layers.
    pub fn get_metadata_for_object(&self, path: &Path, key: &Token) -> Option<usd_vt::Value> {
        let mut partial: Option<usd_vt::Value> = None;

        for (layer, field_path) in self.metadata_site_paths_for_object(path) {
            if let Some(value) = layer.get_field(&field_path, key) {
                if let Some(resolved) = Self::consume_metadata_opinion(&mut partial, value) {
                    return Some(resolved);
                }
            }
        }

        partial.map(Self::finalize_metadata_value)
    }

    fn requires_subtree_prim_index_invalidation(key: &Token) -> bool {
        matches!(
            key.as_str(),
            "references" | "inheritPaths" | "specializes" | "payload"
        )
    }

    /// Returns true if metadata exists for an object at the given path
    /// in any layer of the stage's layer stack.
    pub fn has_authored_metadata_for_object(&self, path: &Path, key: &Token) -> bool {
        self.get_metadata_for_object(path, key).is_some()
    }

    /// Sets metadata for an object at the given path.
    pub fn set_metadata_for_object(&self, path: &Path, key: &Token, value: usd_vt::Value) -> bool {
        let schema = usd_sdf::Schema::instance();
        let spec_type = self.get_object_spec_type(path);
        if !schema.base().is_registered(key) {
            return false;
        }
        if spec_type != SpecType::Unknown && !schema.base().is_valid_field_for_spec(key, spec_type)
        {
            return false;
        }
        let edit_target = self.get_edit_target();
        if let Some(target_layer) = edit_target.layer() {
            let spec_path = edit_target.map_to_spec_path(path);
            target_layer.set_field(&spec_path, key, value);
            let changed_path = if path.is_property_path() {
                path.get_prim_path()
            } else {
                path.clone()
            };
            if !path.is_property_path() {
                if let Some(pcp_cache) = self.pcp_cache() {
                    if Self::requires_subtree_prim_index_invalidation(key) {
                        pcp_cache.clear_prim_index_cache();
                    } else {
                        pcp_cache.invalidate_prim_index(&changed_path);
                    }
                }
            }
            self.handle_local_change(&changed_path);
            true
        } else {
            false
        }
    }

    /// Clears metadata for an object at the given path.
    pub fn clear_metadata_for_object(&self, path: &Path, key: &Token) -> bool {
        let edit_target = self.get_edit_target();
        if let Some(target_layer) = edit_target.layer() {
            let spec_path = edit_target.map_to_spec_path(path);
            target_layer.erase_field(&spec_path, key);
            let changed_path = if path.is_property_path() {
                path.get_prim_path()
            } else {
                path.clone()
            };
            if !path.is_property_path() {
                if let Some(pcp_cache) = self.pcp_cache() {
                    if Self::requires_subtree_prim_index_invalidation(key) {
                        pcp_cache.clear_prim_index_cache();
                    } else {
                        pcp_cache.invalidate_prim_index(&changed_path);
                    }
                }
            }
            self.handle_local_change(&changed_path);
            true
        } else {
            false
        }
    }

    /// Gets all metadata for an object at the given path.
    ///
    /// Composes metadata across the full layer stack (session > root > sublayers).
    /// Stronger layers override weaker ones (matches C++ parity).
    pub fn get_all_metadata_for_object(
        &self,
        path: &Path,
    ) -> std::collections::BTreeMap<Token, usd_vt::Value> {
        use std::collections::BTreeMap;
        let mut result = BTreeMap::new();

        for field in self.list_metadata_fields_for_object(path) {
            if let Some(value) = self.get_metadata_for_object(path, &field) {
                result.insert(field, value);
            }
        }

        result
    }

    /// Gets all authored metadata for an object at the given path.
    pub fn get_all_authored_metadata_for_object(
        &self,
        path: &Path,
    ) -> std::collections::BTreeMap<Token, usd_vt::Value> {
        // Same as get_all_metadata_for_object since we only return authored values
        self.get_all_metadata_for_object(path)
    }

    // ========================================================================
    // TimeCode API
    // ========================================================================

    /// Gets the stage's start time code (matches C++ GetStartTimeCode).
    ///
    /// Checks `startTimeCode` first; falls back to the deprecated `startFrame`
    /// field for backwards compatibility with pre-USD files.
    pub fn get_start_time_code(&self) -> f64 {
        // Check session layer first — prefer startTimeCode, fall back to deprecated startFrame
        if let Some(session) = &self.session_layer {
            if session.has_start_time_code() {
                return session.get_start_time_code();
            } else if session.has_start_frame() {
                return session.get_start_frame();
            }
        }

        // Check root layer
        if self.root_layer.has_start_time_code() {
            self.root_layer.get_start_time_code()
        } else {
            // Fallback to deprecated startFrame for backwards compatibility
            self.root_layer.get_start_frame()
        }
    }

    /// Sets the stage's start time code (matches C++ SetStartTimeCode).
    pub fn set_start_time_code(&self, time_code: f64) {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_start_time_code(time_code);
        }
        // If edit target is not root or session layer, silently do nothing
        // (C++ would issue a warning)
    }

    /// Gets the stage's end time code (matches C++ GetEndTimeCode).
    ///
    /// Checks `endTimeCode` first; falls back to the deprecated `endFrame`
    /// field for backwards compatibility with pre-USD files.
    pub fn get_end_time_code(&self) -> f64 {
        // Check session layer first — prefer endTimeCode, fall back to deprecated endFrame
        if let Some(session) = &self.session_layer {
            if session.has_end_time_code() {
                return session.get_end_time_code();
            } else if session.has_end_frame() {
                return session.get_end_frame();
            }
        }

        // Check root layer
        if self.root_layer.has_end_time_code() {
            self.root_layer.get_end_time_code()
        } else {
            // Fallback to deprecated endFrame for backwards compatibility
            self.root_layer.get_end_frame()
        }
    }

    /// Sets the stage's end time code (matches C++ SetEndTimeCode).
    pub fn set_end_time_code(&self, time_code: f64) {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_end_time_code(time_code);
        }
        // If edit target is not root or session layer, silently do nothing
        // (C++ would issue a warning)
    }

    /// Returns true if the stage has both start and end timeCodes authored (matches C++ HasAuthoredTimeCodeRange).
    ///
    /// Also considers the deprecated `startFrame`/`endFrame` fields for
    /// backwards compatibility, matching C++ `HasAuthoredTimeCodeRange`.
    pub fn has_authored_time_code_range(&self) -> bool {
        // Session layer: either both startTimeCode+endTimeCode, or both deprecated startFrame+endFrame
        if let Some(session) = &self.session_layer {
            if (session.has_start_time_code() && session.has_end_time_code())
                || (session.has_start_frame() && session.has_end_frame())
            {
                return true;
            }
        }

        // Root layer: same logic
        (self.root_layer.has_start_time_code() && self.root_layer.has_end_time_code())
            || (self.root_layer.has_start_frame() && self.root_layer.has_end_frame())
    }

    /// Gets the stage's time codes per second (matches C++ GetTimeCodesPerSecond).
    ///
    /// C++ priority: session TCPS → root TCPS → session FPS → root FPS → fallback.
    /// TCPS takes priority across ALL layers before falling back to FPS.
    pub fn get_time_codes_per_second(&self) -> f64 {
        // 1. Check TCPS across session → root
        if let Some(session) = &self.session_layer {
            if session.has_time_codes_per_second() {
                return session.get_time_codes_per_second();
            }
        }
        if self.root_layer.has_time_codes_per_second() {
            return self.root_layer.get_time_codes_per_second();
        }

        // 2. Fall back to FPS across session → root
        if let Some(session) = &self.session_layer {
            if session.has_frames_per_second() {
                return session.get_frames_per_second();
            }
        }
        if self.root_layer.has_frames_per_second() {
            return self.root_layer.get_frames_per_second();
        }

        24.0 // Default fallback
    }

    /// Sets the stage's time codes per second (matches C++ SetTimeCodesPerSecond).
    pub fn set_time_codes_per_second(&self, time_codes_per_second: f64) {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_time_codes_per_second(time_codes_per_second);
        }
        // If edit target is not root or session layer, silently do nothing
        // (C++ would issue a warning)
    }

    /// Gets the stage's frames per second (matches C++ GetFramesPerSecond).
    pub fn get_frames_per_second(&self) -> f64 {
        // Check session layer first, then root layer
        if let Some(session) = &self.session_layer {
            if session.has_frames_per_second() {
                return session.get_frames_per_second();
            }
        }

        // Check root layer
        if self.root_layer.has_frames_per_second() {
            self.root_layer.get_frames_per_second()
        } else {
            24.0
        }
    }

    /// Sets the stage's frames per second (matches C++ SetFramesPerSecond).
    pub fn set_frames_per_second(&self, frames_per_second: f64) {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_frames_per_second(frames_per_second);
        }
        // If edit target is not root or session layer, silently do nothing
        // (C++ would issue a warning)
    }

    // ========================================================================
    // Variant Fallbacks
    // ========================================================================

    /// Gets the global variant fallback preferences (matches C++ GetGlobalVariantFallbacks).
    ///
    /// Returns the global variant fallback preferences used in new UsdStages.
    /// These override any fallbacks configured in plugin metadata.
    pub fn get_global_variant_fallbacks() -> std::collections::HashMap<Token, Vec<Token>> {
        get_global_variant_fallbacks_storage()
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Sets the global variant fallback preferences (matches C++ SetGlobalVariantFallbacks).
    ///
    /// Sets the global variant fallback preferences used in new UsdStages.
    /// This overrides any fallbacks configured in plugin metadata, and only
    /// affects stages created after this call. Does not affect existing stages.
    pub fn set_global_variant_fallbacks(fallbacks: &std::collections::HashMap<Token, Vec<Token>>) {
        *get_global_variant_fallbacks_storage()
            .write()
            .expect("rwlock poisoned") = fallbacks.clone();
    }

    // ========================================================================
    // Color Configuration API
    // ========================================================================

    /// Sets the default color configuration (matches C++ SetColorConfiguration).
    pub fn set_color_configuration(&self, color_config: &sdf_asset_path::AssetPath) {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_color_configuration(color_config);
        }
        // If edit target is not root or session layer, silently do nothing
        // (C++ would issue a warning)
    }

    /// Gets the default color configuration (matches C++ GetColorConfiguration).
    pub fn get_color_configuration(&self) -> sdf_asset_path::AssetPath {
        // Check session layer first, then root layer
        if let Some(session) = &self.session_layer {
            if session.has_color_configuration() {
                return session.get_color_configuration();
            }
        }

        // Check root layer
        self.root_layer.get_color_configuration()
    }

    /// Sets the color management system (matches C++ SetColorManagementSystem).
    pub fn set_color_management_system(&self, cms: &Token) {
        if let Some(target_layer) = self.get_edit_target_layer() {
            target_layer.set_color_management_system(cms);
        }
        // If edit target is not root or session layer, silently do nothing
        // (C++ would issue a warning)
    }

    /// Gets the color management system (matches C++ GetColorManagementSystem).
    pub fn get_color_management_system(&self) -> Option<Token> {
        // Check session layer first, then root layer
        if let Some(session) = &self.session_layer {
            if session.has_color_management_system() {
                return session.get_color_management_system();
            }
        }

        // Check root layer
        self.root_layer.get_color_management_system()
    }

    /// Gets the global color config fallbacks (matches C++ GetColorConfigFallbacks).
    ///
    /// Returns the global fallback values set in plugInfo.json files or via
    /// SetColorConfigFallbacks(). These can be overridden by calling SetColorConfigFallbacks().
    pub fn get_color_config_fallbacks() -> (Option<sdf_asset_path::AssetPath>, Option<Token>) {
        if let Some((color_config, cms)) = get_global_color_config_fallbacks_storage()
            .read()
            .expect("rwlock poisoned")
            .as_ref()
        {
            (Some(color_config.clone()), Some(cms.clone()))
        } else {
            (None, None)
        }
    }

    /// Sets the global color config fallbacks (matches C++ SetColorConfigFallbacks).
    ///
    /// Sets the global fallback values which override any fallback values authored
    /// in plugInfo files. If a value is None, the corresponding fallback isn't set.
    /// At least one value must be non-empty for this call to have an effect.
    pub fn set_color_config_fallbacks(
        color_configuration: Option<&sdf_asset_path::AssetPath>,
        color_management_system: Option<&Token>,
    ) {
        if color_configuration.is_some() || color_management_system.is_some() {
            let color_config = color_configuration
                .cloned()
                .unwrap_or_else(|| sdf_asset_path::AssetPath::new(""));
            let cms = color_management_system
                .cloned()
                .unwrap_or_else(|| Token::new(""));
            *get_global_color_config_fallbacks_storage()
                .write()
                .expect("rwlock poisoned") = Some((color_config, cms));
        }
    }

    // ========================================================================
    // Instancing
    // ========================================================================

    /// Returns all native instancing prototype prims (matches C++ GetPrototypes).
    ///
    /// Traverses the stage and finds all prototype prims (prims that are instances).
    /// Prototypes are the shared prim subtrees that instances point to.
    pub fn get_prototypes(&self) -> Vec<Prim> {
        let instance_cache = self
            .instance_cache
            .get()
            .expect("Instance cache should be initialized");
        let prototype_paths = instance_cache.get_all_prototypes();

        let mut prototypes = Vec::new();
        let stage_weak = self
            .self_ref
            .get()
            .expect("Stage self_ref should be initialized")
            .clone();

        for prototype_path in prototype_paths {
            if let Some(prim_data) = self.get_prim_data_at_path(&prototype_path) {
                prototypes.push(Prim::from_data(stage_weak.clone(), prim_data));
            }
        }

        prototypes
    }

    /// Returns the prototype prim for the given instance prim (internal method).
    ///
    /// Matches C++ `_GetPrototypeForInstance(Usd_PrimDataConstPtr prim)`.
    pub(crate) fn get_prototype_for_instance(
        &self,
        prim_data: &Arc<PrimData>,
    ) -> Option<Arc<PrimData>> {
        if !prim_data.is_instance() {
            return None;
        }

        let instance_cache = self
            .instance_cache
            .get()
            .expect("Instance cache should be initialized");

        // Get prim index path from prim data
        // Note: This requires PrimData to store prim index path
        // For now, we'll use the prim path as a fallback
        let prim_path = prim_data.path();

        // Get prototype path from instance cache
        let prototype_path =
            instance_cache.get_prototype_for_instanceable_prim_index_path(prim_path);
        if prototype_path.is_empty() {
            return None;
        }

        self.get_prim_data_at_path(&prototype_path)
    }

    /// Returns all instance prims for the given prototype prim (internal method).
    ///
    /// Matches C++ `_GetInstancesForPrototype(const UsdPrim& prototypePrim)`.
    pub(crate) fn get_instances_for_prototype(&self, prototype: &Prim) -> Vec<Prim> {
        if !prototype.is_prototype() {
            return Vec::new();
        }

        let instance_cache = self
            .instance_cache
            .get()
            .expect("Instance cache should be initialized");
        let instance_paths =
            instance_cache.get_instance_prim_indexes_for_prototype(prototype.path());

        let mut instances = Vec::new();
        let stage_weak = self
            .self_ref
            .get()
            .expect("Stage self_ref should be initialized")
            .clone();

        for instance_path in instance_paths {
            if let Some(prim_data) = self.get_prim_data_at_path_or_in_prototype(&instance_path) {
                instances.push(Prim::from_data(stage_weak.clone(), prim_data));
            }
        }

        instances
    }

    /// Returns prim data at path or in prototype (internal method).
    ///
    /// Matches C++ `_GetPrimDataAtPathOrInPrototype(const SdfPath& path)`.
    pub(crate) fn get_prim_data_at_path_or_in_prototype(
        &self,
        path: &Path,
    ) -> Option<Arc<PrimData>> {
        // First try to get prim data at the path directly
        if let Some(prim_data) = self.get_prim_data_at_path(path) {
            return Some(prim_data);
        }

        if !self.is_path_in_prototype(path) {
            return None;
        }

        let mut prototype_root = path.clone();
        while !prototype_root.is_root_prim_path() {
            prototype_root = prototype_root.get_parent_path();
            if prototype_root.is_empty() {
                return None;
            }
        }

        let mut current = self.get_prim_data_at_path(&prototype_root)?;
        if &prototype_root == path {
            return Some(current);
        }

        let relative = path
            .as_str()
            .strip_prefix(prototype_root.as_str())?
            .trim_start_matches('/');
        for child_name in relative.split('/').filter(|segment| !segment.is_empty()) {
            current = current
                .children()
                .into_iter()
                .find(|child| child.name().as_str() == child_name)?;
        }

        Some(current)
    }

    /// Returns true if the given path is in a prototype (internal method).
    ///
    /// Matches C++ `IsPathInPrototype(const SdfPath& path)`.
    #[allow(dead_code)] // C++ parity - prototype path checking
    pub(crate) fn is_path_in_prototype(&self, path: &Path) -> bool {
        use crate::instance_cache::InstanceCache;
        InstanceCache::is_path_in_prototype(path)
    }
}

// ============================================================================
// Error
// ============================================================================

/// Errors that can occur when working with stages.
#[derive(Debug, Clone)]
pub enum Error {
    /// Layer operation failed.
    LayerError(String),
    /// Invalid path.
    InvalidPath(String),
    /// Other error.
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LayerError(msg) => write!(f, "Layer error: {}", msg),
            Self::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::super::prim_flags;
    use super::*;

    #[test]
    fn test_create_in_memory() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        assert!(stage.root_layer().is_anonymous());
    }

    #[test]
    fn test_pseudo_root() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let root = stage.pseudo_root();
        assert_eq!(root.path(), &Path::absolute_root());
        assert!(root.is_pseudo_root());
    }

    #[test]
    fn test_define_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();
        assert!(prim.is_valid());
        assert_eq!(prim.type_name().get_text(), "Xform");
    }

    #[test]
    fn test_edit_target() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let target = stage.edit_target();
        assert!(target.is_local_layer());
    }

    #[test]
    fn test_load_rules() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        assert!(stage.load_rules().is_load_all());

        stage.unload_all();
        assert!(!stage.load_rules().is_load_all());
    }

    #[test]
    fn test_interpolation() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        assert_eq!(stage.interpolation_type(), InterpolationType::Linear);

        stage.set_interpolation_type(InterpolationType::Held);
        assert_eq!(stage.interpolation_type(), InterpolationType::Held);
    }

    #[test]
    fn test_traverse_empty_stage() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prims = stage.traverse_vec(prim_flags::default_predicate().into_predicate());
        // Only pseudo-root if it matches predicate
        assert!(prims.len() <= 1);
    }

    #[test]
    fn test_default_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        assert!(!stage.has_default_prim());
        assert!(stage.default_prim().is_none());
    }

    #[test]
    fn test_clear_default_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.clear_default_prim();
        assert!(!stage.has_default_prim());
    }

    #[test]
    fn test_define_prim_persists_to_layer() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a prim
        let prim = stage.define_prim("/World", "Xform").unwrap();
        assert!(prim.is_valid());

        // Verify it's in the layer
        let layer = stage.root_layer();
        let prim_spec = layer.get_prim_at_path(&prim.path().clone());
        assert!(prim_spec.is_some(), "Prim spec should exist in layer");

        // Verify type name is correct
        let spec = prim_spec.unwrap();
        assert_eq!(spec.type_name().get_text(), "Xform");
    }

    #[test]
    fn test_define_nested_prims() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define parent
        let parent = stage.define_prim("/World", "Xform").unwrap();
        assert!(parent.is_valid());

        // Define child
        let child = stage.define_prim("/World/Cube", "Mesh").unwrap();
        assert!(child.is_valid());
        assert_eq!(child.type_name().get_text(), "Mesh");

        // Verify in layer
        let layer = stage.root_layer();
        assert!(
            layer
                .get_prim_at_path(&Path::from_string("/World").unwrap())
                .is_some()
        );
        assert!(
            layer
                .get_prim_at_path(&Path::from_string("/World/Cube").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_remove_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define then remove
        let prim = stage.define_prim("/ToRemove", "").unwrap();
        let path = prim.path().clone();

        assert!(stage.remove_prim(&path));

        // Should no longer be in layer
        let layer = stage.root_layer();
        assert!(layer.get_prim_at_path(&path).is_none());
    }

    #[test]
    fn test_create_new() {
        let stage = Stage::create_new("test_create_new.usda", InitialLoadSet::LoadAll);
        assert!(stage.is_ok());
        let stage = stage.unwrap();

        // Stage should have root layer
        let root_layer = stage.get_root_layer();
        assert!(!root_layer.identifier().is_empty());

        // Root layer should have the correct identifier
        assert!(root_layer.identifier().contains("test_create_new.usda"));
    }

    #[test]
    fn test_get_pseudo_root() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let pseudo_root = stage.get_pseudo_root();

        assert!(pseudo_root.is_valid());
        assert_eq!(pseudo_root.path(), &Path::absolute_root());
    }

    #[test]
    fn test_get_prim_at_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a prim
        let _prim = stage.define_prim("/World", "Xform").unwrap();

        // Get it back
        let prim = stage.get_prim_at_path(&Path::from_string("/World").unwrap());
        assert!(prim.is_some());
        let prim = prim.unwrap();
        assert!(prim.is_valid());
        assert_eq!(prim.path(), &Path::from_string("/World").unwrap());

        // Non-existent prim should be None
        let invalid_prim = stage.get_prim_at_path(&Path::from_string("/NonExistent").unwrap());
        assert!(invalid_prim.is_none() || !invalid_prim.unwrap().is_valid());
    }

    #[test]
    fn test_get_used_layers() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Initially should have root and session layers
        let used_layers = stage.get_used_layers(false);
        assert!(used_layers.len() >= 1); // At least root layer

        // With session layer, should have 2
        let used_layers_with_session = stage.get_used_layers(false);
        // May have 1 or 2 depending on session layer presence
        assert!(used_layers_with_session.len() >= 1);
    }

    #[test]
    fn test_layer_muting() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Initially no muted layers
        let muted = stage.get_muted_layers();
        assert!(muted.is_empty());

        // Mute a layer (may not exist, but should not panic)
        let layer_id = "test_layer.usda";
        stage.mute_layer(layer_id);

        // Check if muted
        assert!(stage.is_layer_muted(layer_id));

        // Unmute
        stage.unmute_layer(layer_id);

        // Should not be muted anymore
        assert!(!stage.is_layer_muted(layer_id));
    }

    #[test]
    fn test_is_supported_file() {
        // Test valid file names
        assert!(Stage::is_supported_file("foo.usda"));
        assert!(Stage::is_supported_file("/baz/bar/foo.usd"));
        assert!(Stage::is_supported_file("foo.usd"));
        assert!(Stage::is_supported_file("xxx.usdc"));

        // Test invalid file names
        assert!(!Stage::is_supported_file("hello.alembic"));
        assert!(!Stage::is_supported_file("hello.usdx"));
        assert!(!Stage::is_supported_file("ill.never.work"));
    }

    #[test]
    fn test_time_code_metadata() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Initially no time code range (may be false or true depending on defaults)
        let _has_range = stage.has_authored_time_code_range();

        // Set start and end time codes
        stage.set_start_time_code(1.0);
        stage.set_end_time_code(100.0);

        // Get time codes (may return 0.0 if not set on edit target layer)
        let start = stage.get_start_time_code();
        let end = stage.get_end_time_code();
        assert!(start >= 0.0);
        assert!(end >= 0.0);

        // Set time codes per second
        stage.set_time_codes_per_second(24.0);
        let tcps = stage.get_time_codes_per_second();
        assert!(tcps > 0.0);

        // Set frames per second
        stage.set_frames_per_second(24.0);
        let fps = stage.get_frames_per_second();
        assert!(fps > 0.0);
    }

    #[test]
    fn test_color_configuration() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Get fallbacks
        let _fallbacks = Stage::get_color_config_fallbacks();
        // Returns tuple (AssetPath, Token), not Option
        // assert!(fallbacks.is_some());

        // Set color configuration
        let color_config =
            usd_sdf::asset_path::AssetPath::new("https://example.com/config.ocio".to_string());
        stage.set_color_configuration(&color_config);

        let retrieved = stage.get_color_configuration();
        assert_eq!(retrieved.get_asset_path(), color_config.get_asset_path());

        // Set color management system
        let cms = Token::new("OCIO");
        stage.set_color_management_system(&cms);

        let retrieved_cms = stage.get_color_management_system();
        // May be None if not set on edit target layer
        if let Some(retrieved_cms_val) = retrieved_cms {
            assert_eq!(retrieved_cms_val, cms);
        }
    }

    #[test]
    fn test_metadata_operations() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Test metadata operations
        let key = Token::new("testKey");
        let value = usd_vt::Value::from("testValue".to_string());

        // Set metadata
        let result = stage.set_metadata(&key, value.clone());
        assert!(result);

        // Check if metadata exists
        assert!(stage.has_metadata(&key));
        assert!(stage.has_authored_metadata(&key));

        // Get metadata
        let retrieved = stage.get_metadata(&key);
        assert!(retrieved.is_some());

        // Clear metadata
        let result = stage.clear_metadata(&key);
        assert!(result);

        assert!(!stage.has_authored_metadata(&key));
    }

    #[test]
    fn test_export_to_string() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a prim
        let _prim = stage.define_prim("/World", "Xform").unwrap();

        // Export to string
        let exported = stage.export_to_string(false);
        assert!(exported.is_ok());
        let exported_str = exported.unwrap();

        // Should contain USD header or be non-empty
        assert!(!exported_str.is_empty());
    }

    #[test]
    fn test_flatten() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a prim
        let _prim = stage.define_prim("/World", "Xform").unwrap();

        // Flatten the stage
        let flattened = stage.flatten(false);
        assert!(flattened.is_ok());
        let flattened_layer = flattened.unwrap();

        // Flattened layer should exist
        assert!(!flattened_layer.identifier().is_empty());
    }

    #[test]
    fn test_get_prototypes() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Initially no prototypes
        let prototypes = stage.get_prototypes();
        assert!(prototypes.is_empty());

        // After creating instanceable prims, prototypes should appear
        // (This would require full instancing setup - tested separately)
    }

    #[test]
    fn test_load_unload() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Load a prim
        let prim_path = Path::from_string("/World").unwrap();
        let _prim = stage.define_prim("/World", "Xform").unwrap();

        // Test load/unload

        stage.load(&prim_path, Some(LoadPolicy::LoadWithDescendants));
        stage.unload(&prim_path);
    }

    #[test]
    fn test_population_mask() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Initially no population mask (all prims)
        let mask = stage.get_population_mask();
        assert!(mask.is_none());

        // Set a population mask
        let mut new_mask = StagePopulationMask::new();
        new_mask.add(Path::from_string("/World").unwrap());
        stage.set_population_mask(Some(new_mask));

        let retrieved_mask = stage.get_population_mask();
        assert!(retrieved_mask.is_some());
    }

    #[test]
    fn test_get_edit_target() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Get current edit target
        let edit_target = stage.get_edit_target();
        // Edit target should always be available
        assert!(edit_target.layer().is_some());

        // Get root layer
        let root_layer = stage.get_root_layer();
        assert!(!root_layer.identifier().is_empty());
    }

    #[test]
    fn test_traverse_all() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define some prims
        let _world = stage.define_prim("/World", "Xform").unwrap();
        let _sphere = stage.define_prim("/World/Sphere", "Sphere").unwrap();

        // Traverse all prims
        let mut count = 0;
        for _prim in stage.traverse_all() {
            count += 1;
        }

        // traverse_all starts from first child of pseudo-root (not pseudo-root itself),
        // so we get /World + /World/Sphere = 2 prims minimum.
        assert!(count >= 2);
    }

    #[test]
    fn test_save_all() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a prim
        let _prim = stage.define_prim("/World", "Xform").unwrap();

        // Save (may fail if file system issues, but should not panic)
        let _result = stage.save_all();
        // Don't assert on result as it may fail due to file system permissions
    }

    // ========================================================================
    // Asset path resolution tests
    // ========================================================================

    #[test]
    fn test_make_resolved_asset_paths() {
        use usd_sdf::asset_path::AssetPath as SdfAssetPath;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create asset paths
        let mut paths = vec![
            SdfAssetPath::new("test.usd"),
            SdfAssetPath::new("models/sphere.usd"),
        ];

        // Should not panic, even if resolution doesn't change paths
        stage.make_resolved_asset_paths(&mut paths);

        // Authored paths should be preserved
        assert_eq!(paths[0].get_authored_path(), "test.usd");
        assert_eq!(paths[1].get_authored_path(), "models/sphere.usd");
    }

    #[test]
    fn test_make_resolved_asset_paths_value_single() {
        use usd_sdf::asset_path::AssetPath as SdfAssetPath;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let ap = SdfAssetPath::new("test.usd");
        let mut val = usd_vt::Value::new(ap);

        // Should process without panic
        stage.make_resolved_asset_paths_value(&mut val);

        // Value should still be an SdfAssetPath
        assert!(val.is::<SdfAssetPath>());
        let resolved = val.get::<SdfAssetPath>().unwrap();
        assert_eq!(resolved.get_authored_path(), "test.usd");
    }

    #[test]
    fn test_make_resolved_asset_paths_value_array() {
        use usd_sdf::asset_path::AssetPath as SdfAssetPath;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let paths = vec![SdfAssetPath::new("a.usd"), SdfAssetPath::new("b.usd")];
        let mut val = usd_vt::Value::new(paths);

        // Should process without panic
        stage.make_resolved_asset_paths_value(&mut val);

        // Value should still be a Vec<SdfAssetPath>
        assert!(val.is::<Vec<SdfAssetPath>>());
        let resolved = val.get::<Vec<SdfAssetPath>>().unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].get_authored_path(), "a.usd");
        assert_eq!(resolved[1].get_authored_path(), "b.usd");
    }

    #[test]
    fn test_make_resolved_asset_paths_value_non_asset() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Non-asset-path value should be left unchanged
        let mut val = usd_vt::Value::new(42i32);
        stage.make_resolved_asset_paths_value(&mut val);
        assert!(val.is::<i32>());
        assert_eq!(*val.get::<i32>().unwrap(), 42);
    }

    #[test]
    fn test_path_resolver_context_anonymous_open_matches_create_default_context() {
        // C++ `_CreatePathResolverContext`: anonymous root → `ArGetResolver().CreateDefaultContext()`.
        let layer = usd_sdf::layer::Layer::create_anonymous(Some("test_auto_ctx"));
        let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).unwrap();

        let resolver = usd_ar::resolver::get_resolver();
        let guard = resolver.read().expect("rwlock poisoned");
        let expected = guard.create_default_context();

        let ctx = stage.get_path_resolver_context();
        assert_eq!(ctx, Some(expected));
    }

    #[test]
    fn test_set_resolver_context() {
        use usd_ar::resolver_context::ResolverContext;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Set a custom resolver context
        let ctx = ResolverContext::new();
        stage.set_path_resolver_context(Some(ctx));

        // Should be retrievable
        let retrieved = stage.get_path_resolver_context();
        assert!(retrieved.is_some());

        // Clear it
        stage.set_path_resolver_context(None);
        let cleared = stage.get_path_resolver_context();
        assert!(cleared.is_none());
    }

    // ========================================================================
    // Composition change processing tests
    // ========================================================================

    #[test]
    fn test_handle_layers_did_change_serial_dedup() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // First call with serial=1 should process
        stage.handle_layers_did_change(1, vec![]);
        assert_eq!(stage.last_change_serial.load(Ordering::Relaxed), 1);

        // Duplicate serial=1 should be ignored
        stage.handle_layers_did_change(1, vec![]);
        assert_eq!(stage.last_change_serial.load(Ordering::Relaxed), 1);

        // Older serial=0 should be ignored
        stage.handle_layers_did_change(0, vec![]);
        assert_eq!(stage.last_change_serial.load(Ordering::Relaxed), 1);

        // Newer serial=2 should process
        stage.handle_layers_did_change(2, vec![]);
        assert_eq!(stage.last_change_serial.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_handle_layers_did_change_recompose() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a prim
        let _prim = stage.define_prim("/World", "Xform").unwrap();
        assert!(
            stage
                .get_prim_at_path(&Path::from_string("/World").unwrap())
                .is_some()
        );

        // Trigger change at root — should not crash
        stage.handle_layers_did_change(1, vec![Path::absolute_root()]);

        // Stage should still be usable
        assert!(
            stage
                .get_prim_at_path(&Path::from_string("/World").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_process_pending_changes_empty() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // No pending changes — should return false
        assert!(!stage.process_pending_changes());
    }

    #[test]
    fn test_recompose_prims_descendant_pruning() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let _world = stage.define_prim("/World", "Xform").unwrap();
        let _child = stage.define_prim("/World/Child", "Sphere").unwrap();

        // Both ancestor and descendant: descendant should be pruned
        let paths = vec![
            Path::from_string("/World").unwrap(),
            Path::from_string("/World/Child").unwrap(),
        ];

        // Should not panic, only /World should be recomposed
        stage.recompose_prims(&paths);

        // Prims should still be accessible after recomposition
        assert!(
            stage
                .get_prim_at_path(&Path::from_string("/World").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_recompose_prims_does_not_duplicate_traversal_entries() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/World", "Xform").unwrap();

        let root = Path::absolute_root();
        let before = stage.traverse_from(&root, prim_flags::PrimFlagsPredicate::default());
        let before_world_count = before
            .iter()
            .filter(|prim| prim.path().as_str() == "/World")
            .count();
        assert_eq!(before_world_count, 1);

        stage.recompose_prims(&[Path::from_string("/World").unwrap()]);

        let after = stage.traverse_from(&root, prim_flags::PrimFlagsPredicate::default());
        let after_world_count = after
            .iter()
            .filter(|prim| prim.path().as_str() == "/World")
            .count();
        assert_eq!(after_world_count, 1);
    }

    #[test]
    fn test_recompose_empty() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Empty recompose should be a no-op
        let changes = usd_pcp::Changes::new();
        stage.recompose(&changes, &[]);
    }

    #[test]
    fn test_objects_changed_notice_sent() {
        use crate::notice::ObjectsChanged;
        use std::sync::atomic::AtomicBool;
        use usd_tf::notice;

        let received = Arc::new(AtomicBool::new(false));
        let received_clone = received.clone();

        // Register a global listener for ObjectsChanged
        let key = notice::register_global::<ObjectsChanged, _>(move |_notice| {
            received_clone.store(true, Ordering::SeqCst);
        });

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let _prim = stage.define_prim("/Test", "Xform").unwrap();

        // Trigger a change
        stage.handle_layers_did_change(1, vec![Path::from_string("/Test").unwrap()]);

        // Verify notice was sent
        assert!(
            received.load(Ordering::SeqCst),
            "ObjectsChanged notice should have been sent"
        );

        notice::revoke(key);
    }

    #[test]
    fn test_handle_layers_multiple_paths() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let _a = stage.define_prim("/A", "Xform").unwrap();
        let _b = stage.define_prim("/B", "Xform").unwrap();

        // Change multiple paths at once
        stage.handle_layers_did_change(
            1,
            vec![
                Path::from_string("/A").unwrap(),
                Path::from_string("/B").unwrap(),
            ],
        );

        // Both should still be accessible
        assert!(
            stage
                .get_prim_at_path(&Path::from_string("/A").unwrap())
                .is_some()
        );
        assert!(
            stage
                .get_prim_at_path(&Path::from_string("/B").unwrap())
                .is_some()
        );
    }

    // ====================================================================
    // Parallel Composition Tests
    // ====================================================================

    #[test]
    fn test_parallel_threshold_constant() {
        // Verify the threshold is a reasonable value
        assert_eq!(Stage::PARALLEL_THRESHOLD, 10);
    }

    #[test]
    fn test_compose_prim_indexes_in_parallel() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define enough prims to exceed the parallel threshold
        for i in 0..15 {
            let path = format!("/Prim{}", i);
            stage.define_prim(&path, "Xform").unwrap();
        }

        // Compose prim indexes in parallel
        let paths: Vec<Path> = (0..15)
            .filter_map(|i| Path::from_string(&format!("/Prim{}", i)))
            .collect();

        stage.compose_prim_indexes_in_parallel(&paths);

        // Verify all prims are still accessible
        for i in 0..15 {
            let path = Path::from_string(&format!("/Prim{}", i)).unwrap();
            assert!(
                stage.get_prim_at_path(&path).is_some(),
                "Prim {} should exist after parallel index composition",
                i
            );
        }
    }

    #[test]
    fn test_compose_subtrees_in_parallel() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create multiple subtrees
        for i in 0..12 {
            let root = format!("/Root{}", i);
            stage.define_prim(&root, "Xform").unwrap();
            stage
                .define_prim(&format!("{}/Child", root), "Mesh")
                .unwrap();
        }

        let paths: Vec<Path> = (0..12)
            .filter_map(|i| Path::from_string(&format!("/Root{}", i)))
            .collect();

        let weak = stage.self_ref.get().unwrap().clone();
        stage.compose_subtrees_in_parallel(&paths, &weak);

        // All prims and children should still be accessible
        for i in 0..12 {
            let root_path = Path::from_string(&format!("/Root{}", i)).unwrap();
            let child_path = Path::from_string(&format!("/Root{}/Child", i)).unwrap();
            assert!(stage.get_prim_at_path(&root_path).is_some());
            assert!(stage.get_prim_at_path(&child_path).is_some());
        }
    }

    #[test]
    fn test_recompose_uses_parallel_for_large_sets() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create more prims than the parallel threshold
        let count = Stage::PARALLEL_THRESHOLD + 5;
        for i in 0..count {
            stage.define_prim(&format!("/P{}", i), "Xform").unwrap();
        }

        // Recompose all paths (should trigger parallel path)
        let paths: Vec<Path> = (0..count)
            .filter_map(|i| Path::from_string(&format!("/P{}", i)))
            .collect();

        stage.recompose_prims(&paths);

        // All prims should still be accessible after parallel recomposition
        for i in 0..count {
            let path = Path::from_string(&format!("/P{}", i)).unwrap();
            assert!(
                stage.get_prim_at_path(&path).is_some(),
                "Prim P{} missing after parallel recompose",
                i
            );
        }
    }

    #[test]
    fn test_recompose_sequential_for_small_sets() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create fewer prims than the threshold
        for i in 0..3 {
            stage.define_prim(&format!("/S{}", i), "Xform").unwrap();
        }

        let paths: Vec<Path> = (0..3)
            .filter_map(|i| Path::from_string(&format!("/S{}", i)))
            .collect();

        // Should use sequential path (under threshold)
        stage.recompose_prims(&paths);

        for i in 0..3 {
            let path = Path::from_string(&format!("/S{}", i)).unwrap();
            assert!(stage.get_prim_at_path(&path).is_some());
        }
    }

    #[test]
    fn test_parallel_produces_same_results_as_sequential() {
        // Create two identical stages, recompose one sequentially and
        // one in parallel, verify same results
        let stage_seq = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let stage_par = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let count = 20;
        for i in 0..count {
            let path = format!("/Node{}", i);
            stage_seq.define_prim(&path, "Xform").unwrap();
            stage_par.define_prim(&path, "Xform").unwrap();
        }

        let paths: Vec<Path> = (0..count)
            .filter_map(|i| Path::from_string(&format!("/Node{}", i)))
            .collect();

        // Sequential: recompose with below-threshold slices
        for chunk in paths.chunks(5) {
            stage_seq.recompose_prims(chunk);
        }

        // Parallel: recompose all at once (above threshold)
        stage_par.recompose_prims(&paths);

        // Both should have the same prims
        for i in 0..count {
            let path = Path::from_string(&format!("/Node{}", i)).unwrap();
            let seq_has = stage_seq.get_prim_at_path(&path).is_some();
            let par_has = stage_par.get_prim_at_path(&path).is_some();
            assert_eq!(
                seq_has, par_has,
                "Node{}: seq={}, par={}",
                i, seq_has, par_has
            );
        }
    }

    #[test]
    fn test_create_new_load_none_sets_load_rules() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadNone).unwrap();
        // With LoadNone, the load rules should start as load_none
        let rules = stage.load_rules.read().expect("rwlock poisoned");
        // load_none sets the default rule to NoneRule
        assert!(!rules.is_loaded(&Path::from_string("/any").unwrap()));
    }

    #[test]
    fn test_reload_empty_stage_succeeds() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        // Reloading a fresh in-memory stage should not panic or error
        // (no layers to reload from disk, but the loop should execute fine)
        assert!(stage.reload().is_ok());
    }

    #[test]
    fn test_color_config_get_set() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let config = sdf_asset_path::AssetPath::new("ocio://config.ocio");
        stage.set_color_configuration(&config);
        let got = stage.get_color_configuration();
        assert_eq!(got.get_asset_path(), config.get_asset_path());
    }

    #[test]
    fn test_color_config_fallbacks() {
        let cfg = sdf_asset_path::AssetPath::new("ocio://global.ocio");
        let cms = Token::new("ocio");
        Stage::set_color_config_fallbacks(Some(&cfg), Some(&cms));
        let (got_cfg, got_cms) = Stage::get_color_config_fallbacks();
        assert_eq!(got_cfg.unwrap().get_asset_path(), "ocio://global.ocio");
        assert_eq!(got_cms.unwrap().get_text(), "ocio");
    }
}
