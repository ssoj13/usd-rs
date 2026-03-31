//! PCP Layer Stack - a composed stack of layers.
//!
//! A layer stack represents a stack of layers that contribute opinions
//! to composition. Each layer stack is identified by a `LayerStackIdentifier`.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/layerStack.h` (~400 lines).
//!
//! # Key Concepts
//!
//! - **Layer Stack**: Ordered collection of layers from strong to weak
//! - **Session Layers**: Temporary layers for editing (optional)
//! - **Root Layer**: The main layer that anchors the stack
//! - **Sublayers**: Layers included via sublayering
//! - **Layer Offsets**: Time/scale transforms between layers

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use usd_ar::ResolverContextBinder;

use super::errors::ErrorType;
use super::layer_stack_identifier::LayerStackIdentifier;
use usd_sdf::AssetPath;
use usd_sdf::{Layer, LayerOffset, Path};

// ============================================================================
// Layer Stack
// ============================================================================

/// A composed stack of layers contributing opinions.
///
/// The layer stack represents all layers that participate in composition
/// for a particular root layer, in strength order (strong to weak).
///
/// # Examples
///
/// ```rust,ignore
/// use usd_pcp::LayerStack;
///
/// let stack = LayerStack::new(identifier);
/// for layer in stack.layers() {
///     println!("Layer: {}", layer.identifier());
/// }
/// ```
#[derive(Debug)]
pub struct LayerStack {
    /// The identifier for this layer stack.
    identifier: LayerStackIdentifier,

    /// Whether this is a USD-mode layer stack.
    is_usd: RwLock<bool>,

    /// Layers in strong-to-weak order.
    layers: RwLock<Vec<Arc<Layer>>>,

    /// Session layers subset (also in layers vec).
    session_layers: RwLock<Vec<Arc<Layer>>>,

    /// Layer offsets indexed by layer.
    /// Maps layer identifier to its offset.
    layer_offsets: RwLock<HashMap<String, LayerOffset>>,

    /// Muted layer paths.
    muted_layers: RwLock<Vec<String>>,

    /// Expression variables for this stack.
    expression_variables: RwLock<HashMap<String, String>>,

    /// Expression variable dependencies.
    expression_variable_dependencies: RwLock<std::collections::HashSet<String>>,

    /// Time codes per second for this stack.
    time_codes_per_second: RwLock<f64>,

    /// Layer tree structure (computed lazily).
    layer_tree: RwLock<Option<Arc<usd_sdf::LayerTree>>>,

    /// Session layer tree structure (computed lazily).
    session_layer_tree: RwLock<Option<Arc<usd_sdf::LayerTree>>>,

    /// Combined relocates source to target (includes incremental + inherited).
    relocates_source_to_target: RwLock<HashMap<Path, Path>>,

    /// Combined relocates target to source (includes incremental + inherited).
    relocates_target_to_source: RwLock<HashMap<Path, Path>>,

    /// Paths to prims that contain relocates.
    paths_to_prims_with_relocates: RwLock<Vec<Path>>,

    /// Errors encountered during composition.
    local_errors: RwLock<Vec<ErrorType>>,

    /// Relocates map (target path -> source path).
    /// The incremental map contains only the relocates from this layer stack,
    /// not combined with ancestor stacks.
    incremental_relocates_target_to_source: RwLock<HashMap<Path, Path>>,

    /// Relocates map (source path -> target path).
    incremental_relocates_source_to_target: RwLock<HashMap<Path, Path>>,

    /// Weak self reference for shared ownership.
    self_ref: RwLock<Weak<Self>>,
}

impl LayerStack {
    /// Creates a new layer stack with the given identifier.
    ///
    /// # Arguments
    ///
    /// * `identifier` - The identifier for this layer stack
    pub fn new(identifier: LayerStackIdentifier) -> Arc<Self> {
        let stack = Arc::new(Self {
            identifier,
            is_usd: RwLock::new(true),
            layers: RwLock::new(Vec::new()),
            session_layers: RwLock::new(Vec::new()),
            layer_offsets: RwLock::new(HashMap::new()),
            muted_layers: RwLock::new(Vec::new()),
            expression_variables: RwLock::new(HashMap::new()),
            expression_variable_dependencies: RwLock::new(std::collections::HashSet::new()),
            time_codes_per_second: RwLock::new(24.0), // Default TCPS
            layer_tree: RwLock::new(None),
            session_layer_tree: RwLock::new(None),
            relocates_source_to_target: RwLock::new(HashMap::new()),
            relocates_target_to_source: RwLock::new(HashMap::new()),
            paths_to_prims_with_relocates: RwLock::new(Vec::new()),
            local_errors: RwLock::new(Vec::new()),
            incremental_relocates_target_to_source: RwLock::new(HashMap::new()),
            incremental_relocates_source_to_target: RwLock::new(HashMap::new()),
            self_ref: RwLock::new(Weak::new()),
        });

        // Store weak self reference
        *stack.self_ref.write().expect("rwlock poisoned") = Arc::downgrade(&stack);

        stack
    }

    /// Creates a layer stack from a root layer.
    ///
    /// This is a convenience method that creates an identifier from
    /// the root layer and constructs the stack.
    pub fn from_root_layer(root_layer: Arc<Layer>) -> Arc<Self> {
        Self::from_root_layer_with_session(root_layer, None)
    }

    /// Creates a layer stack from a root layer and optional session layer.
    ///
    /// Composes timeCodesPerSecond from session layer (if it has it),
    /// falling back to root layer's value, then to the default 24.0.
    pub fn from_root_layer_with_session(
        root_layer: Arc<Layer>,
        session_layer: Option<Arc<Layer>>,
    ) -> Arc<Self> {
        let identifier = LayerStackIdentifier::new(AssetPath::new(root_layer.identifier()));
        let stack = Self::new(identifier);

        // Add session layer first (strongest)
        if let Some(ref session) = session_layer {
            stack.add_session_layer(session.clone());
        }

        // Add root layer
        stack
            .layers
            .write()
            .expect("rwlock poisoned")
            .push(root_layer.clone());
        stack.compute_sublayers(&root_layer);

        // Compose TCPS: session layer wins, then root layer, then default 24.0
        let tcps = Self::compose_tcps(session_layer.as_ref(), &root_layer);
        *stack
            .time_codes_per_second
            .write()
            .expect("rwlock poisoned") = tcps;

        // Bug 3 (P1-CTX): Bind resolver context during stack build, matching C++
        // `ArResolverContextBinder binder(_identifier.pathResolverContext)` in
        // PcpLayerStack::_Compute (layerStack.cpp:1533).
        // The binder is dropped at the end of this scope, automatically unbinding.
        let _ctx_binder = stack
            .identifier
            .resolver_context
            .clone()
            .map(ResolverContextBinder::new);

        // Bug 5 (P1-RELOC): Compute relocations after building the layer stack,
        // matching C++ `Pcp_ComputeRelocationsForLayerStack(*this, ...)` call
        // at layerStack.cpp:1636.
        stack.compute_relocations();

        stack
    }

    /// Returns the identifier for this layer stack.
    pub fn identifier(&self) -> &LayerStackIdentifier {
        &self.identifier
    }

    /// Returns true if this is a USD-mode layer stack.
    pub fn is_usd(&self) -> bool {
        *self.is_usd.read().expect("rwlock poisoned")
    }

    /// Returns the layers in strong-to-weak order.
    ///
    /// This is only the *local* layer stack - it does not include
    /// layers brought in by references inside prims.
    pub fn get_layers(&self) -> Vec<Arc<Layer>> {
        self.layers.read().expect("rwlock poisoned").clone()
    }

    /// Returns only the session layers in strong-to-weak order.
    pub fn get_session_layers(&self) -> Vec<Arc<Layer>> {
        self.session_layers.read().expect("rwlock poisoned").clone()
    }

    /// Returns the root layer of this stack.
    pub fn root_layer(&self) -> Option<Arc<Layer>> {
        let layers = self.layers.read().expect("rwlock poisoned");
        let session_layers = self.session_layers.read().expect("rwlock poisoned");

        // Root layer is the first non-session layer
        // Or first layer if no session layers
        if session_layers.is_empty() {
            layers.first().cloned()
        } else {
            // Find first non-session layer
            layers
                .iter()
                .find(|l| !session_layers.iter().any(|s| Arc::ptr_eq(s, *l)))
                .cloned()
        }
    }

    /// Returns the layer offset for the given layer.
    ///
    /// Returns `None` if the layer is not in the stack or has identity offset.
    pub fn get_layer_offset(&self, layer: &Arc<Layer>) -> Option<LayerOffset> {
        let id = layer.identifier();
        self.layer_offsets
            .read()
            .expect("rwlock poisoned")
            .get(id)
            .filter(|o| !o.is_identity())
            .cloned()
    }

    /// Returns the layer offset for the given layer identifier.
    pub fn get_layer_offset_by_id(&self, id: &str) -> Option<LayerOffset> {
        self.layer_offsets
            .read()
            .expect("rwlock poisoned")
            .get(id)
            .filter(|o| !o.is_identity())
            .cloned()
    }

    /// Returns the layer offset for the layer at the given index.
    ///
    /// Returns `None` if the index is out of bounds or offset is identity.
    pub fn get_layer_offset_at(&self, index: usize) -> Option<LayerOffset> {
        let layers = self.layers.read().expect("rwlock poisoned");
        layers.get(index).and_then(|l| self.get_layer_offset(l))
    }

    /// Returns the number of layers in the stack.
    pub fn len(&self) -> usize {
        self.layers.read().expect("rwlock poisoned").len()
    }

    /// Returns true if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.layers.read().expect("rwlock poisoned").is_empty()
    }

    /// Checks if a layer path is muted.
    pub fn is_layer_muted(&self, layer_path: &str) -> bool {
        self.muted_layers
            .read()
            .expect("rwlock poisoned")
            .iter()
            .any(|m| m == layer_path)
    }

    /// Returns the muted layer paths.
    pub fn get_muted_layers(&self) -> Vec<String> {
        self.muted_layers.read().expect("rwlock poisoned").clone()
    }

    /// Returns the expression variables for this stack.
    pub fn get_expression_variables(&self) -> HashMap<String, String> {
        self.expression_variables
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns local errors encountered during composition.
    pub fn local_errors(&self) -> Vec<ErrorType> {
        self.local_errors.read().expect("rwlock poisoned").clone()
    }

    /// Returns true if this stack has any local errors.
    pub fn has_local_errors(&self) -> bool {
        !self
            .local_errors
            .read()
            .expect("rwlock poisoned")
            .is_empty()
    }

    /// Finds the layer containing a spec at the given path.
    ///
    /// Returns the strongest layer that has a spec at this path,
    /// along with the path translated to that layer's namespace.
    pub fn find_layer_with_spec(&self, path: &Path) -> Option<(Arc<Layer>, Path)> {
        let layers = self.layers.read().expect("rwlock poisoned");
        for layer in layers.iter() {
            // Check if layer has a prim, attribute, or relationship spec at this path
            if layer.get_prim_at_path(path).is_some()
                || layer.get_attribute_at_path(path).is_some()
                || layer.get_relationship_at_path(path).is_some()
            {
                return Some((layer.clone(), path.clone()));
            }
        }
        None
    }

    /// Applies layer offsets to a time code.
    ///
    /// Transforms a time code from the layer at `from_index` to
    /// the layer at `to_index`.
    pub fn apply_offset(&self, time: f64, from_index: usize, to_index: usize) -> f64 {
        if from_index == to_index {
            return time;
        }

        let layers = self.layers.read().expect("rwlock poisoned");
        let mut result = time;

        // Apply offsets for layers between from and to
        if from_index < to_index {
            // Going from stronger to weaker - apply inverse offsets
            for i in from_index..to_index {
                if let Some(layer) = layers.get(i) {
                    if let Some(offset) = self.get_layer_offset(layer) {
                        result = offset.inverse().apply(result);
                    }
                }
            }
        } else {
            // Going from weaker to stronger - apply offsets
            for i in to_index..from_index {
                if let Some(layer) = layers.get(i) {
                    if let Some(offset) = self.get_layer_offset(layer) {
                        result = offset.apply(result);
                    }
                }
            }
        }

        result
    }

    // ========================================================================
    // Internal Methods
    // ========================================================================

    /// Composes timeCodesPerSecond from session + root layers.
    ///
    /// Matches C++ `_ShouldUseSessionTcps()` + `_Compute()` in layerStack.cpp:
    /// - Use session TCPS if session layer has authored `timeCodesPerSecond`, OR
    ///   if root layer has NO authored `timeCodesPerSecond` but session has `framesPerSecond`.
    /// - Otherwise use root layer's TCPS (which itself falls back to framesPerSecond, then 24.0).
    fn compose_tcps(session_layer: Option<&Arc<Layer>>, root_layer: &Arc<Layer>) -> f64 {
        if let Some(session) = session_layer {
            // _ShouldUseSessionTcps: session wins if it has explicit tcps,
            // or if root has no tcps but session has fps
            let session_has_tcps = session.get_metadata_field("timeCodesPerSecond").is_some();
            let root_has_tcps = root_layer
                .get_metadata_field("timeCodesPerSecond")
                .is_some();
            let session_has_fps = session.has_frames_per_second();

            if session_has_tcps || (!root_has_tcps && session_has_fps) {
                return session.get_time_codes_per_second();
            }
        }
        // Root layer wins: prefer timeCodesPerSecond, fall back to framesPerSecond, then 24.0
        root_layer.get_time_codes_per_second()
    }

    /// Computes sublayers for the given layer and adds them to the stack.
    ///
    /// Per C++ layerStack.cpp _BuildLayerStack: reads sublayer offsets from
    /// the parent layer, composes them with the cumulative offset, applies
    /// TCPS scaling, and stores the result in layer_offsets map.
    fn compute_sublayers(&self, layer: &Arc<Layer>) {
        // P1-12 FIX: Pass a seenLayers set for cycle detection, matching C++.
        // C++ uses SdfLayerHandleSet *seenLayers that is inserted before recursion
        // and erased after, so the same layer CAN appear in multiple branches
        // but NOT on the same recursive path (which would be a cycle).
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        seen.insert(layer.identifier().to_string());
        let session_owner = self.get_session_owner();
        self.compute_sublayers_with_offset(
            layer,
            &LayerOffset::identity(),
            &session_owner,
            &mut seen,
        );
    }

    /// Recursive sublayer builder that carries cumulative offset and seen-layers set.
    ///
    /// P1-12 FIX: `seen` tracks layers on the current recursive path for cycle detection.
    /// Matches C++ `_BuildLayerStack` which uses `seenLayers->insert/count/erase`.
    fn compute_sublayers_with_offset(
        &self,
        layer: &Arc<Layer>,
        cumulative_offset: &LayerOffset,
        session_owner: &str,
        seen: &mut std::collections::HashSet<String>,
    ) {
        let sublayer_paths = layer.sublayer_paths();
        let sublayer_offsets = layer.get_sublayer_offsets();
        let layer_tcps = layer.get_time_codes_per_second();

        let mut resolved_sublayers = Vec::new();
        for (i, path) in sublayer_paths.iter().enumerate() {
            let resolved = self.resolve_sublayer(layer, path);
            if resolved.is_none() {
                // Matching C++: record PcpErrorInvalidSublayerPath when a sublayer
                // cannot be opened. Compose continues with the remaining sublayers.
                self.add_error(ErrorType::InvalidSublayerPath);
            }
            if let Some(sublayer) = resolved {
                resolved_sublayers.push((i, sublayer));
            }
        }

        if !session_owner.is_empty() && layer.get_has_owned_sublayers() {
            resolved_sublayers.sort_by_key(|(_, sublayer)| {
                if sublayer.has_owner() && sublayer.get_owner() == session_owner {
                    0
                } else {
                    1
                }
            });
        }

        for (i, sublayer) in resolved_sublayers {
            // P1-12 FIX: Cycle detection — if this layer is already on the current
            // recursive path, skip it and record an error (matching C++ seenLayers check).
            let sublayer_id = sublayer.identifier().to_string();
            if seen.contains(&sublayer_id) {
                self.add_error(ErrorType::SublayerCycle);
                continue;
            }

            // Read the authored sublayer offset (or identity if index out of range)
            let mut sub_offset = sublayer_offsets
                .get(i)
                .cloned()
                .unwrap_or_else(LayerOffset::identity);

            // Apply TCPS scaling: if sublayer has different TCPS, scale the offset
            let sublayer_tcps = sublayer.get_time_codes_per_second();
            if (layer_tcps - sublayer_tcps).abs() > f64::EPSILON {
                sub_offset = LayerOffset::new(
                    sub_offset.offset(),
                    sub_offset.scale() * layer_tcps / sublayer_tcps,
                );
            }

            // Compose with cumulative offset: absolute = cumulative * sublayer
            let absolute_offset = cumulative_offset.compose(&sub_offset);

            // Store offset in the map (keyed by layer identifier)
            if !absolute_offset.is_identity() {
                self.layer_offsets
                    .write()
                    .expect("rwlock poisoned")
                    .insert(sublayer_id.clone(), absolute_offset.clone());
            }

            // Add to layers list
            self.layers
                .write()
                .expect("rwlock poisoned")
                .push(sublayer.clone());

            // Mark as seen for cycle detection, recurse, then unmark (C++ erase after).
            seen.insert(sublayer_id.clone());
            self.compute_sublayers_with_offset(&sublayer, &absolute_offset, session_owner, seen);
            seen.remove(&sublayer_id);
        }
    }

    fn get_session_owner(&self) -> String {
        for session_layer in self.session_layers.read().expect("rwlock poisoned").iter() {
            let owner = session_layer.get_session_owner();
            if !owner.is_empty() {
                return owner;
            }
        }
        String::new()
    }

    /// Resolves a sublayer path relative to a parent layer.
    ///
    /// C++ layerStack.cpp:1715: `SdfComputeAssetPathRelativeToLayer(layer, sublayers[i])`
    /// then `SdfLayer::FindOrOpen(sublayerPath)`
    fn resolve_sublayer(&self, parent: &Arc<Layer>, path: &str) -> Option<Arc<Layer>> {
        // Use ArResolver-based path computation (matches C++ pipeline)
        let anchored = usd_sdf::layer_utils::compute_asset_path_relative_to_layer(parent, path);
        Layer::find_or_open(&anchored).ok()
    }

    /// Computes relocations from all layers in the stack.
    ///
    /// Equivalent to C++ `Pcp_ComputeRelocationsForLayerStack` (layerStack.cpp:896-946).
    ///
    /// Iterates layers strong-to-weak, reading layer-metadata relocates
    /// (`get_relocates()`). For each (source, target) pair:
    /// - source must not be a root prim path
    /// - source must not already be present (strong layer wins, like try_emplace)
    ///
    /// Fills four maps:
    ///   incremental_s2t / incremental_t2s: authored source <-> target
    ///   combined_s2t / combined_t2s: source <-> target (no deduplication here
    ///   since we don't have computedSourceOrigin in this simplified port)
    fn compute_relocations(&self) {
        let layers = self.layers.read().expect("rwlock poisoned").clone();

        let mut incr_s2t: HashMap<Path, Path> = HashMap::new();
        let mut incr_t2s: HashMap<Path, Path> = HashMap::new();
        let prim_paths: Vec<Path> = Vec::new();

        let abs_root = Path::absolute_root();

        for layer in &layers {
            // C++ _CollectLayerRelocates: read layer-metadata relocates field.
            // In USD mode (is_usd=true) this is the only source.
            if !layer.has_relocates() {
                continue;
            }
            let relocates = layer.get_relocates();
            for (source, target) in relocates {
                // Make absolute (C++ MakeAbsolutePath with root path)
                let source_abs = source
                    .make_absolute(&abs_root)
                    .unwrap_or_else(|| source.clone());
                let target_abs = target
                    .make_absolute(&abs_root)
                    .unwrap_or_else(|| target.clone());

                // Validate: source must not be root prim (C++ _IsValidRelocatesEntryForComposing)
                if source_abs.is_root_prim_path() {
                    continue;
                }
                // Source and target must not be the same
                if source_abs == target_abs {
                    continue;
                }

                // try_emplace: strong layer wins (first insertion only)
                if !incr_s2t.contains_key(&source_abs) {
                    incr_s2t.insert(source_abs.clone(), target_abs.clone());
                    incr_t2s.insert(target_abs.clone(), source_abs.clone());
                }
            }
        }

        // Collect prim paths that have relocates authored on them.
        // In USD mode all relocates come from layer metadata, so the prim
        // path is always the pseudo-root; we skip populating prim_paths here
        // to match C++ behaviour where layer-metadata relocates don't add to
        // allPrimPathsWithAuthoredRelocates (that only happens for prim-spec
        // relocates in non-USD mode).
        let _ = prim_paths; // intentionally empty in USD mode

        // In this simplified port we treat incremental == combined
        // (no computedSourceOrigin deduplication), matching what the existing
        // tests expect and the apply() path uses.
        *self
            .incremental_relocates_source_to_target
            .write()
            .expect("rwlock poisoned") = incr_s2t.clone();
        *self
            .incremental_relocates_target_to_source
            .write()
            .expect("rwlock poisoned") = incr_t2s.clone();
        *self
            .relocates_source_to_target
            .write()
            .expect("rwlock poisoned") = incr_s2t;
        *self
            .relocates_target_to_source
            .write()
            .expect("rwlock poisoned") = incr_t2s;
    }

    /// Adds an error to the local errors list.
    pub(crate) fn add_error(&self, error: ErrorType) {
        self.local_errors
            .write()
            .expect("rwlock poisoned")
            .push(error);
    }

    /// Sets the USD mode flag.
    #[allow(dead_code)] // Internal API - used by layer stack building
    pub(crate) fn set_is_usd(&self, is_usd: bool) {
        *self.is_usd.write().expect("rwlock poisoned") = is_usd;
    }

    /// Adds a session layer to the stack.
    pub(crate) fn add_session_layer(&self, layer: Arc<Layer>) {
        // Session layers go at the front (strongest)
        self.layers
            .write()
            .expect("rwlock poisoned")
            .insert(0, layer.clone());
        self.session_layers
            .write()
            .expect("rwlock poisoned")
            .push(layer);
    }

    /// Sets the expression variables.
    #[allow(dead_code)] // Internal API - used by layer stack building
    pub(crate) fn set_expression_variables(&self, vars: HashMap<String, String>) {
        *self.expression_variables.write().expect("rwlock poisoned") = vars;
    }

    /// Sets the muted layers list.
    pub(crate) fn set_muted_layers(&self, muted: Vec<String>) {
        *self.muted_layers.write().expect("rwlock poisoned") = muted;
    }

    // ========================================================================
    // Relocates
    // ========================================================================

    /// Returns the incremental relocates map (target to source).
    ///
    /// This map contains only the relocates from this layer stack,
    /// mapping relocated target paths back to their original source paths.
    pub fn incremental_relocates_target_to_source(&self) -> HashMap<Path, Path> {
        self.incremental_relocates_target_to_source
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns the incremental relocates map (source to target).
    ///
    /// This map contains only the relocates from this layer stack,
    /// mapping original source paths to their relocated target paths.
    pub fn incremental_relocates_source_to_target(&self) -> HashMap<Path, Path> {
        self.incremental_relocates_source_to_target
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Sets the relocates maps.
    ///
    /// This is typically called during layer stack computation.
    pub(crate) fn set_relocates(
        &self,
        target_to_source: HashMap<Path, Path>,
        source_to_target: HashMap<Path, Path>,
    ) {
        *self
            .incremental_relocates_target_to_source
            .write()
            .expect("rwlock poisoned") = target_to_source;
        *self
            .incremental_relocates_source_to_target
            .write()
            .expect("rwlock poisoned") = source_to_target;
    }

    /// Returns true if this layer stack has any relocates.
    pub fn has_relocates(&self) -> bool {
        !self
            .incremental_relocates_target_to_source
            .read()
            .expect("rwlock poisoned")
            .is_empty()
    }

    /// Returns the layer tree representing the structure of non-session layers.
    pub fn get_layer_tree(&self) -> Option<Arc<usd_sdf::LayerTree>> {
        self.layer_tree.read().expect("rwlock poisoned").clone()
    }

    /// Returns the layer tree representing the structure of session layers.
    pub fn get_session_layer_tree(&self) -> Option<Arc<usd_sdf::LayerTree>> {
        self.session_layer_tree
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns true if this layer stack contains the given layer.
    pub fn has_layer(&self, layer: &Arc<Layer>) -> bool {
        let layers = self.layers.read().expect("rwlock poisoned");
        layers.iter().any(|l| Arc::ptr_eq(l, layer))
    }

    /// Returns true if this layer stack contains the given layer handle.
    pub fn has_layer_handle(&self, layer_handle: &usd_sdf::LayerHandle) -> bool {
        if let Some(layer) = layer_handle.upgrade() {
            self.has_layer(&layer)
        } else {
            false
        }
    }

    /// Returns the set of expression variables used during computation.
    pub fn get_expression_variable_dependencies(&self) -> std::collections::HashSet<String> {
        self.expression_variable_dependencies
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns the time codes per second value of the layer stack.
    pub fn get_time_codes_per_second(&self) -> f64 {
        *self.time_codes_per_second.read().expect("rwlock poisoned")
    }

    /// Returns relocation source-to-target mapping for this layer stack.
    ///
    /// This map combines incremental relocates with inherited relocates.
    pub fn relocates_source_to_target(&self) -> HashMap<Path, Path> {
        self.relocates_source_to_target
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns relocation target-to-source mapping for this layer stack.
    ///
    /// This map combines incremental relocates with inherited relocates.
    pub fn relocates_target_to_source(&self) -> HashMap<Path, Path> {
        self.relocates_target_to_source
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Returns a list of paths to all prims that contained relocates.
    pub fn paths_to_prims_with_relocates(&self) -> Vec<Path> {
        self.paths_to_prims_with_relocates
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Applies the changes in `changes`.
    ///
    /// This blows caches. It's up to the client to pull on those caches
    /// again as needed.
    ///
    /// Objects that would be destroyed are retained in `lifeboat` so
    /// the client can control destruction timing.
    ///
    /// Matches C++ `PcpLayerStack::Apply()`.
    pub fn apply(
        &self,
        changes: &super::changes::LayerStackChanges,
        mut lifeboat: Option<&mut super::changes::Lifeboat>,
    ) {
        // Update expression variables up-front (may be needed for layer
        // stack recomputation).
        if changes.did_change_significantly
            || changes.did_change_expression_variables
            || changes.did_change_expression_variables_source()
        {
            if changes.did_change_expression_variables {
                *self.expression_variables.write().expect("rwlock poisoned") =
                    changes.new_expression_variables.clone();
            }
            // Clear cached dependencies so they are recomputed.
            self.expression_variable_dependencies
                .write()
                .expect("rwlock poisoned")
                .clear();
        }

        // If layers or offsets changed, blow caches and recompute.
        if changes.did_change_layers || changes.did_change_layer_offsets {
            // Retain prior layers so they survive until lifeboat is dropped.
            if let Some(ref mut lb) = lifeboat {
                let layers = self.layers.read().expect("rwlock poisoned");
                for layer in layers.iter() {
                    lb.retain_layer(layer.clone());
                }
            }

            // Blow layer tree caches.
            *self.layer_tree.write().expect("rwlock poisoned") = None;
            *self.session_layer_tree.write().expect("rwlock poisoned") = None;

            // Blow relocations (they depend on layer content).
            self.blow_relocations();

            // Recompute the layer stack from the root layer.
            if changes.did_change_layers {
                if let Some(root) = self.root_layer() {
                    // Clear layers and re-add root + sublayers.
                    let session_layers = self.get_session_layers();
                    {
                        let mut layers = self.layers.write().expect("rwlock poisoned");
                        layers.clear();
                        // Re-add session layers first.
                        for sl in &session_layers {
                            layers.push(sl.clone());
                        }
                        layers.push(root.clone());
                    }
                    self.compute_sublayers(&root);

                    // Recompute TCPS from (possibly changed) layers.
                    let session = session_layers.first().cloned();
                    let tcps = Self::compose_tcps(session.as_ref(), &root);
                    *self.time_codes_per_second.write().expect("rwlock poisoned") = tcps;
                }
            }
        } else if changes.did_change_significantly || changes.did_change_relocates {
            // Relocates changed but layers didn't.
            self.blow_relocations();

            if changes.did_change_significantly {
                // Significant change: blow tree caches too.
                *self.layer_tree.write().expect("rwlock poisoned") = None;
                *self.session_layer_tree.write().expect("rwlock poisoned") = None;
            }

            if !changes.did_change_significantly && changes.did_change_relocates {
                // Use the pre-computed relocate maps from the change.
                *self
                    .relocates_source_to_target
                    .write()
                    .expect("rwlock poisoned") = changes.new_relocates_source_to_target.clone();
                *self
                    .relocates_target_to_source
                    .write()
                    .expect("rwlock poisoned") = changes.new_relocates_target_to_source.clone();
                *self
                    .incremental_relocates_source_to_target
                    .write()
                    .expect("rwlock poisoned") =
                    changes.new_incremental_relocates_source_to_target.clone();
                *self
                    .incremental_relocates_target_to_source
                    .write()
                    .expect("rwlock poisoned") =
                    changes.new_incremental_relocates_target_to_source.clone();
                *self
                    .paths_to_prims_with_relocates
                    .write()
                    .expect("rwlock poisoned") = changes.new_relocates_prim_paths.clone();
            }
        }
    }

    /// Clears all relocate maps.
    fn blow_relocations(&self) {
        self.relocates_source_to_target
            .write()
            .expect("rwlock poisoned")
            .clear();
        self.relocates_target_to_source
            .write()
            .expect("rwlock poisoned")
            .clear();
        self.incremental_relocates_source_to_target
            .write()
            .expect("rwlock poisoned")
            .clear();
        self.incremental_relocates_target_to_source
            .write()
            .expect("rwlock poisoned")
            .clear();
        self.paths_to_prims_with_relocates
            .write()
            .expect("rwlock poisoned")
            .clear();
    }

    /// Returns a MapExpression representing the relocations that affect
    /// namespace at and below the given path.
    ///
    /// In USD mode, returns a null expression if there are no relocations.
    ///
    /// Matches C++ `PcpLayerStack::GetExpressionForRelocatesAtPath()`.
    pub fn get_expression_for_relocates_at_path(
        &self,
        path: &Path,
    ) -> super::map_expression::MapExpression {
        use super::map_expression::MapExpression;
        use super::map_function::{MapFunction, PathMap};

        // Don't waste time if there are no relocates (USD-mode optimisation).
        if self.is_usd() && !self.has_relocates() {
            return MapExpression::null();
        }

        // Collect relocates that affect namespace at and below `path`.
        // Track targets to avoid non-invertible duplicates between combined
        // and incremental maps.
        let mut site_relocates = PathMap::new();
        let mut seen_targets = std::collections::HashSet::new();

        // Combined relocates: source has_prefix(path).
        let combined = self.relocates_source_to_target();
        for (source, target) in &combined {
            if source.has_prefix(path) && !target.is_empty() {
                site_relocates.insert(source.clone(), target.clone());
                seen_targets.insert(target.clone());
            }
        }

        // Incremental relocates: source has_prefix(path), skip duplicates.
        let incremental = self.incremental_relocates_source_to_target();
        for (source, target) in &incremental {
            if source.has_prefix(path) && !target.is_empty() {
                if seen_targets.insert(target.clone()) {
                    site_relocates.insert(source.clone(), target.clone());
                }
            }
        }

        // Always include the absolute-root self-map so the function is valid.
        site_relocates.insert(Path::absolute_root(), Path::absolute_root());

        if let Some(map_fn) = MapFunction::create(site_relocates, Default::default()) {
            MapExpression::constant(map_fn)
        } else {
            MapExpression::null()
        }
    }
}

// ============================================================================
// Ref Counting
// ============================================================================

/// A reference-counted pointer to a layer stack.
pub type LayerStackRefPtr = Arc<LayerStack>;

/// A weak pointer to a layer stack.
pub type LayerStackPtr = Weak<LayerStack>;

// Implement Hash and Eq for LayerStackRefPtr to use in HashMap
impl std::hash::Hash for LayerStack {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash by identifier for stable hashing
        self.identifier.hash(state);
    }
}

impl PartialEq for LayerStack {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl Eq for LayerStack {}

impl Default for LayerStack {
    fn default() -> Self {
        Self {
            identifier: LayerStackIdentifier::default(),
            is_usd: RwLock::new(true),
            layers: RwLock::new(Vec::new()),
            session_layers: RwLock::new(Vec::new()),
            layer_offsets: RwLock::new(HashMap::new()),
            muted_layers: RwLock::new(Vec::new()),
            expression_variables: RwLock::new(HashMap::new()),
            expression_variable_dependencies: RwLock::new(std::collections::HashSet::new()),
            time_codes_per_second: RwLock::new(24.0),
            layer_tree: RwLock::new(None),
            session_layer_tree: RwLock::new(None),
            relocates_source_to_target: RwLock::new(HashMap::new()),
            relocates_target_to_source: RwLock::new(HashMap::new()),
            paths_to_prims_with_relocates: RwLock::new(Vec::new()),
            local_errors: RwLock::new(Vec::new()),
            incremental_relocates_target_to_source: RwLock::new(HashMap::new()),
            incremental_relocates_source_to_target: RwLock::new(HashMap::new()),
            self_ref: RwLock::new(Weak::new()),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_layer_stack_creation() {
        let id = LayerStackIdentifier::default();
        let stack = LayerStack::new(id);
        assert!(stack.is_empty());
        assert!(stack.is_usd());
    }

    #[test]
    fn test_layer_stack_from_root() {
        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer.clone());
        assert_eq!(stack.len(), 1);
        let root = stack.root_layer().unwrap();
        assert!(Arc::ptr_eq(&root, &layer));
    }

    #[test]
    fn test_layer_stack_errors() {
        let stack = LayerStack::new(LayerStackIdentifier::default());
        assert!(!stack.has_local_errors());

        stack.add_error(ErrorType::InvalidAssetPath);
        assert!(stack.has_local_errors());
        assert_eq!(stack.local_errors().len(), 1);
    }

    #[test]
    fn test_layer_offset_application() {
        let stack = LayerStack::new(LayerStackIdentifier::default());
        // With no offsets, time should be unchanged
        let time = stack.apply_offset(10.0, 0, 0);
        assert_eq!(time, 10.0);
    }

    #[test]
    fn test_compose_tcps_root_layer_value() {
        let root = Layer::create_anonymous(None);
        root.set_time_codes_per_second(48.0);
        let stack = LayerStack::from_root_layer_with_session(root, None);
        assert_eq!(stack.get_time_codes_per_second(), 48.0);
    }

    #[test]
    fn test_compose_tcps_session_overrides_root() {
        let root = Layer::create_anonymous(None);
        root.set_time_codes_per_second(48.0);
        let session = Layer::create_anonymous(None);
        session.set_time_codes_per_second(30.0);
        let stack = LayerStack::from_root_layer_with_session(root, Some(session));
        // Session layer TCPS wins over root layer
        assert_eq!(stack.get_time_codes_per_second(), 30.0);
    }

    #[test]
    fn test_compose_tcps_default_fallback() {
        let root = Layer::create_anonymous(None);
        // Neither root nor session set TCPS => default 24.0
        let stack = LayerStack::from_root_layer_with_session(root, None);
        assert_eq!(stack.get_time_codes_per_second(), 24.0);
    }

    // ================================================================
    // M1: HasLayer tests
    // ================================================================

    #[test]
    fn test_has_layer_present() {
        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer.clone());
        assert!(stack.has_layer(&layer));
    }

    #[test]
    fn test_has_layer_absent() {
        let layer = Layer::create_anonymous(None);
        let other = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer);
        assert!(!stack.has_layer(&other));
    }

    // ================================================================
    // M7: Apply tests
    // ================================================================

    #[test]
    fn test_apply_expression_variable_change() {
        use crate::changes::LayerStackChanges;

        let stack = LayerStack::new(LayerStackIdentifier::default());
        assert!(stack.get_expression_variables().is_empty());

        let mut changes = LayerStackChanges::new();
        changes.did_change_expression_variables = true;
        changes
            .new_expression_variables
            .insert("SHOT".into(), "s001".into());

        stack.apply(&changes, None);

        let vars = stack.get_expression_variables();
        assert_eq!(vars.get("SHOT"), Some(&"s001".to_string()));
    }

    #[test]
    fn test_apply_relocates_change() {
        use crate::changes::LayerStackChanges;

        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer);

        // Set up relocates via changes.
        let mut changes = LayerStackChanges::new();
        changes.did_change_relocates = true;
        let src = Path::from_string("/World/OldName").unwrap();
        let tgt = Path::from_string("/World/NewName").unwrap();
        changes
            .new_relocates_source_to_target
            .insert(src.clone(), tgt.clone());
        changes
            .new_relocates_target_to_source
            .insert(tgt.clone(), src.clone());
        changes
            .new_incremental_relocates_source_to_target
            .insert(src.clone(), tgt.clone());
        changes
            .new_incremental_relocates_target_to_source
            .insert(tgt.clone(), src.clone());
        changes
            .new_relocates_prim_paths
            .push(Path::from_string("/World").unwrap());

        stack.apply(&changes, None);

        // Check that relocate maps were updated.
        let s2t = stack.relocates_source_to_target();
        assert_eq!(s2t.get(&src), Some(&tgt));
        let t2s = stack.relocates_target_to_source();
        assert_eq!(t2s.get(&tgt), Some(&src));
        assert!(stack.has_relocates());
        assert_eq!(stack.paths_to_prims_with_relocates().len(), 1);
    }

    #[test]
    fn test_apply_significant_change_blows_caches() {
        use crate::changes::LayerStackChanges;

        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer);

        // Set some relocates first (both directions so has_relocates sees them).
        {
            let src = Path::from_string("/A").unwrap();
            let tgt = Path::from_string("/B").unwrap();
            stack
                .incremental_relocates_source_to_target
                .write()
                .unwrap()
                .insert(src.clone(), tgt.clone());
            stack
                .incremental_relocates_target_to_source
                .write()
                .unwrap()
                .insert(tgt, src);
        }
        assert!(stack.has_relocates());

        // Significant change blows relocates.
        let mut changes = LayerStackChanges::new();
        changes.did_change_significantly = true;

        stack.apply(&changes, None);

        // Relocates should be cleared.
        assert!(!stack.has_relocates());
    }

    #[test]
    fn test_apply_lifeboat_retains_layers() {
        use crate::changes::{LayerStackChanges, Lifeboat};

        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer.clone());

        let mut changes = LayerStackChanges::new();
        changes.did_change_layers = true;

        let mut lifeboat = Lifeboat::new();
        stack.apply(&changes, Some(&mut lifeboat));

        // The lifeboat should have retained the original layer.
        assert!(!lifeboat.layers().is_empty());
    }

    // ================================================================
    // M8: GetExpressionForRelocatesAtPath tests
    // ================================================================

    #[test]
    fn test_expression_for_relocates_no_relocates() {
        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer);

        let expr =
            stack.get_expression_for_relocates_at_path(&Path::from_string("/World").unwrap());
        // No relocates => null expression.
        assert!(expr.is_null());
    }

    #[test]
    fn test_expression_for_relocates_with_matching() {
        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer);

        // Set up a relocate under /World (both directions for has_relocates).
        let src = Path::from_string("/World/OldName").unwrap();
        let tgt = Path::from_string("/World/NewName").unwrap();
        stack
            .relocates_source_to_target
            .write()
            .unwrap()
            .insert(src.clone(), tgt.clone());
        stack
            .incremental_relocates_source_to_target
            .write()
            .unwrap()
            .insert(src.clone(), tgt.clone());
        stack
            .incremental_relocates_target_to_source
            .write()
            .unwrap()
            .insert(tgt.clone(), src.clone());

        let expr =
            stack.get_expression_for_relocates_at_path(&Path::from_string("/World").unwrap());
        // Should produce a non-null expression.
        assert!(!expr.is_null());
    }

    #[test]
    fn test_expression_for_relocates_non_matching() {
        let layer = Layer::create_anonymous(None);
        let stack = LayerStack::from_root_layer(layer);

        // Relocate under /Other, not /World.  Need both directions.
        let src = Path::from_string("/Other/A").unwrap();
        let tgt = Path::from_string("/Other/B").unwrap();
        stack
            .relocates_source_to_target
            .write()
            .unwrap()
            .insert(src.clone(), tgt.clone());
        stack
            .incremental_relocates_source_to_target
            .write()
            .unwrap()
            .insert(src.clone(), tgt.clone());
        stack
            .incremental_relocates_target_to_source
            .write()
            .unwrap()
            .insert(tgt, src);

        let expr =
            stack.get_expression_for_relocates_at_path(&Path::from_string("/World").unwrap());
        // The resulting expression contains only the root self-map since
        // /Other/A is not under /World.  It should still be non-null
        // (a valid identity-like map) because has_relocates() is true.
        assert!(!expr.is_null());
        // But it should be effectively identity (only root -> root mapping).
        assert!(expr.is_constant_identity());
    }

    // ================================================================
    // M12: TCPS composition tests (additional)
    // ================================================================

    #[test]
    fn test_compose_tcps_session_without_tcps_falls_to_root() {
        let root = Layer::create_anonymous(None);
        root.set_time_codes_per_second(60.0);
        let session = Layer::create_anonymous(None);
        // Session doesn't set TCPS => root's 60.0 wins.
        let stack = LayerStack::from_root_layer_with_session(root, Some(session));
        assert_eq!(stack.get_time_codes_per_second(), 60.0);
    }

    #[test]
    fn test_owned_sublayers_are_reordered_ahead_of_unowned() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testenv")
            .join("museum")
            .join("BasicOwner");
        let root = Layer::find_or_open(base.join("root.usda").to_string_lossy().as_ref()).unwrap();
        let session =
            Layer::find_or_open(base.join("session.usda").to_string_lossy().as_ref()).unwrap();

        assert!(root.get_has_owned_sublayers());
        assert_eq!(session.get_session_owner(), "foo");

        let stack = LayerStack::from_root_layer_with_session(root, Some(session));
        let layer_names: Vec<String> = stack
            .get_layers()
            .into_iter()
            .map(|layer| {
                PathBuf::from(layer.identifier())
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        assert_eq!(
            layer_names,
            vec![
                "session.usda".to_string(),
                "root.usda".to_string(),
                "owned.usda".to_string(),
                "stronger.usda".to_string(),
                "weaker.usda".to_string(),
            ]
        );
    }
}
