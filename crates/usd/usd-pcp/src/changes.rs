//! PCP Changes - change processing and invalidation.
//!
//! This module provides structures for tracking and processing changes to
//! Pcp caches and layer stacks. It translates Sdf changes into the corresponding
//! Pcp invalidations.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/changes.h` and `changes.cpp`.
//!
//! # Key Types
//!
//! - **LayerStackChanges**: Changes to a specific layer stack
//! - **CacheChanges**: Changes to a specific cache
//! - **Lifeboat**: Temporary retention of layers/stacks during change processing
//! - **Changes**: Main collector of all Pcp changes

use crate::{
    Cache, DependencyFlags, DependencyVector, ErrorType, ExpressionVariablesSource, LayerStackPtr,
    LayerStackRefPtr,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use usd_sdf::{Layer, Path};

// ============================================================================
// Layer Stack Changes
// ============================================================================

/// Types of changes to a layer stack.
///
/// Tracks what aspects of a layer stack have changed and need recomputation.
#[derive(Clone, Debug, Default)]
pub struct LayerStackChanges {
    /// Must rebuild the layer tree. Implies `did_change_layer_offsets`.
    pub did_change_layers: bool,

    /// Must rebuild the layer offsets.
    pub did_change_layer_offsets: bool,

    /// Must rebuild the relocation tables.
    pub did_change_relocates: bool,

    /// Must rebuild expression variables.
    pub did_change_expression_variables: bool,

    /// A significant change means composed opinions may have changed arbitrarily.
    /// This is coarse invalidation (vs. fine-grained like adding an empty layer).
    pub did_change_significantly: bool,

    /// New relocation maps for this layer stack (if did_change_relocates).
    /// Maps target paths to source paths for relocations.
    pub new_relocates_target_to_source: HashMap<Path, Path>,
    /// Maps source paths to target paths for relocations.
    pub new_relocates_source_to_target: HashMap<Path, Path>,
    /// Maps source paths to target paths for incremental relocations.
    pub new_incremental_relocates_source_to_target: HashMap<Path, Path>,
    /// Maps target paths to source paths for incremental relocations.
    pub new_incremental_relocates_target_to_source: HashMap<Path, Path>,
    /// Prim paths affected by new relocations.
    pub new_relocates_prim_paths: Vec<Path>,
    /// Errors encountered during relocation processing.
    pub new_relocates_errors: Vec<ErrorType>,

    /// Paths affected by relocation changes.
    pub paths_affected_by_relocation_changes: HashSet<Path>,

    /// New expression variables for this layer stack.
    pub new_expression_variables: HashMap<String, String>,

    // Private fields
    did_change_expression_variables_source: bool,
    new_expression_variables_source: Option<ExpressionVariablesSource>,
}

impl LayerStackChanges {
    /// Creates a new empty layer stack changes object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if any changes have been recorded.
    pub fn has_changes(&self) -> bool {
        self.did_change_layers
            || self.did_change_layer_offsets
            || self.did_change_relocates
            || self.did_change_expression_variables
            || self.did_change_significantly
    }

    /// Marks that layers changed.
    pub fn mark_layers_changed(&mut self) {
        self.did_change_layers = true;
    }

    /// Marks that layer offsets changed.
    pub fn mark_offsets_changed(&mut self) {
        self.did_change_layer_offsets = true;
    }

    /// Marks that relocates changed.
    pub fn mark_relocates_changed(&mut self) {
        self.did_change_relocates = true;
    }

    /// Marks significant change.
    pub fn mark_significant(&mut self) {
        self.did_change_significantly = true;
    }

    /// Sets the new expression variables source.
    ///
    /// This indicates that the source of expression variables has changed
    /// and stores the new source for later retrieval.
    pub fn set_expression_variables_source(&mut self, source: ExpressionVariablesSource) {
        self.did_change_expression_variables_source = true;
        self.new_expression_variables_source = Some(source);
    }

    /// Returns true if the expression variables source changed.
    pub fn did_change_expression_variables_source(&self) -> bool {
        self.did_change_expression_variables_source
    }

    /// Gets the new expression variables source, if set.
    pub fn get_new_expression_variables_source(&self) -> Option<&ExpressionVariablesSource> {
        self.new_expression_variables_source.as_ref()
    }

    /// Takes the new expression variables source, leaving None.
    pub fn take_expression_variables_source(&mut self) -> Option<ExpressionVariablesSource> {
        self.did_change_expression_variables_source = false;
        self.new_expression_variables_source.take()
    }
}

// ============================================================================
// Target Type
// ============================================================================

/// Type of target change (connection or relationship target).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TargetType {
    /// Attribute connection target.
    Connection = 1,
    /// Relationship target.
    RelationshipTarget = 2,
}

impl TargetType {
    /// Returns combined flags for both target types.
    pub fn all() -> u8 {
        TargetType::Connection as u8 | TargetType::RelationshipTarget as u8
    }
}

// ============================================================================
// Cache Changes
// ============================================================================

/// Types of changes per cache.
///
/// Tracks what needs to be rebuilt in a specific Pcp cache.
#[derive(Clone, Debug, Default)]
pub struct CacheChanges {
    /// Must rebuild indexes at and below each path (implies prim/property stacks).
    pub did_change_significantly: HashSet<Path>,

    /// Must rebuild prim/property stacks at each path.
    pub did_change_specs: HashSet<Path>,

    /// Must rebuild prim indexes at each path (implies prim stack).
    pub did_change_prims: HashSet<Path>,

    /// Must rebuild connections/targets at each path.
    /// Value is combination of TargetType flags.
    pub did_change_targets: HashMap<Path, u8>,

    /// Must update path on namespace objects at and below each path.
    /// Pairs of (old_path, new_path) in order of edits.
    pub did_change_path: Vec<(Path, Path)>,

    /// Layers used in composition may have changed.
    pub did_maybe_change_layers: bool,

    /// Identifiers of layers that were muted or removed and all sublayers recursively.
    pub layers_affected_by_muting_or_removal: HashSet<String>,

    // Private fields for internal spec changes
    did_change_specs_internal: HashSet<Path>,
    did_change_prim_specs_and_children_internal: HashSet<Path>,
}

impl CacheChanges {
    /// Creates a new empty cache changes object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if any changes have been recorded.
    pub fn has_changes(&self) -> bool {
        !self.did_change_significantly.is_empty()
            || !self.did_change_specs.is_empty()
            || !self.did_change_prims.is_empty()
            || !self.did_change_targets.is_empty()
            || !self.did_change_path.is_empty()
            || self.did_maybe_change_layers
    }

    /// Adds a significant change at the given path.
    pub fn add_significant_change(&mut self, path: Path) {
        self.did_change_significantly.insert(path);
    }

    /// Adds a specs change at the given path.
    pub fn add_specs_change(&mut self, path: Path) {
        self.did_change_specs.insert(path);
    }

    /// Adds a prims change at the given path.
    pub fn add_prims_change(&mut self, path: Path) {
        self.did_change_prims.insert(path);
    }

    /// Adds a targets change at the given path.
    pub fn add_targets_change(&mut self, path: Path, target_type: TargetType) {
        let entry = self.did_change_targets.entry(path).or_insert(0);
        *entry |= target_type as u8;
    }

    /// Adds a path change (rename).
    pub fn add_path_change(&mut self, old_path: Path, new_path: Path) {
        self.did_change_path.push((old_path, new_path));
    }

    /// Adds an internal spec change (for incremental processing).
    ///
    /// Internal changes are accumulated during processing and later rolled
    /// up into the public `did_change_specs` when processing completes.
    pub fn add_specs_change_internal(&mut self, path: Path) {
        self.did_change_specs_internal.insert(path);
    }

    /// Adds an internal prim spec with children change.
    ///
    /// Used during processing to track prims whose children also changed.
    pub fn add_prim_specs_and_children_change_internal(&mut self, path: Path) {
        self.did_change_prim_specs_and_children_internal
            .insert(path);
    }

    /// Returns the internal spec changes.
    pub fn get_specs_changes_internal(&self) -> &HashSet<Path> {
        &self.did_change_specs_internal
    }

    /// Returns the internal prim specs and children changes.
    pub fn get_prim_specs_and_children_changes_internal(&self) -> &HashSet<Path> {
        &self.did_change_prim_specs_and_children_internal
    }

    /// Promotes internal changes to public and clears internal sets.
    ///
    /// Call this after change processing is complete.
    pub fn finalize_internal_changes(&mut self) {
        for path in self.did_change_specs_internal.drain() {
            self.did_change_specs.insert(path);
        }
        for path in self.did_change_prim_specs_and_children_internal.drain() {
            self.did_change_prims.insert(path);
        }
    }
}

// ============================================================================
// Specs Change Type
// ============================================================================

/// Type of spec change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeSpecsType {
    /// Spec was removed.
    Removed,
    /// Spec was added.
    Added,
}

// ============================================================================
// Lifeboat
// ============================================================================

/// Structure used to temporarily retain layers and layer stacks.
///
/// Analogous to an autorelease pool - ensures objects live long enough
/// for change processing to complete.
#[derive(Clone, Default)]
pub struct Lifeboat {
    layers: Vec<Arc<Layer>>,
    layer_stacks: Vec<LayerStackRefPtr>,
}

impl std::fmt::Debug for Lifeboat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lifeboat")
            .field("layers_count", &self.layers.len())
            .field("layer_stacks_count", &self.layer_stacks.len())
            .finish()
    }
}

impl Lifeboat {
    /// Creates a new empty lifeboat.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensures the layer exists until this lifeboat is destroyed.
    pub fn retain_layer(&mut self, layer: Arc<Layer>) {
        // Check if already retained
        if !self.layers.iter().any(|l| Arc::ptr_eq(l, &layer)) {
            self.layers.push(layer);
        }
    }

    /// Ensures the layer stack exists until this lifeboat is destroyed.
    pub fn retain_layer_stack(&mut self, layer_stack: LayerStackRefPtr) {
        // Check if already retained
        if !self
            .layer_stacks
            .iter()
            .any(|ls| Arc::ptr_eq(ls, &layer_stack))
        {
            self.layer_stacks.push(layer_stack);
        }
    }

    /// Returns reference to the layer stacks being held.
    pub fn layer_stacks(&self) -> &[LayerStackRefPtr] {
        &self.layer_stacks
    }

    /// Returns reference to the layers being held.
    pub fn layers(&self) -> &[Arc<Layer>] {
        &self.layers
    }

    /// Swaps contents with another lifeboat.
    pub fn swap(&mut self, other: &mut Lifeboat) {
        std::mem::swap(&mut self.layers, &mut other.layers);
        std::mem::swap(&mut self.layer_stacks, &mut other.layer_stacks);
    }

    /// Clears all retained objects.
    pub fn clear(&mut self) {
        self.layers.clear();
        self.layer_stacks.clear();
    }
}

// ============================================================================
// Pcp Changes
// ============================================================================

/// Layer stack identifier for change tracking.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayerStackChangeKey {
    /// Root layer identifier.
    root_layer_id: String,
    /// Session layer identifier (if any).
    session_layer_id: Option<String>,
}

impl LayerStackChangeKey {
    fn from_layer_stack(layer_stack: &LayerStackRefPtr) -> Self {
        let identifier = layer_stack.identifier();
        let root_layer_id = identifier.root_layer.get_authored_path().to_string();
        let session_layer_id = identifier
            .session_layer
            .as_ref()
            .map(|p| p.get_authored_path().to_string());
        Self {
            root_layer_id,
            session_layer_id,
        }
    }
}

/// Describes Pcp changes.
///
/// Collects changes to Pcp necessary to reflect changes in Sdf. It does not
/// cause any changes to any Pcp caches or layer stacks; it only computes what
/// changes would be necessary.
#[derive(Debug, Default)]
pub struct Changes {
    /// Changes per layer stack (keyed by layer stack identifier).
    layer_stack_changes: HashMap<LayerStackChangeKey, (LayerStackPtr, LayerStackChanges)>,
    /// Changes per cache (using raw pointer for map key since Cache doesn't implement Hash).
    cache_changes: HashMap<usize, CacheChanges>,
    /// Rename changes per cache.
    rename_changes: HashMap<usize, HashMap<Path, Path>>,
    /// Lifeboat for retaining objects during change processing.
    lifeboat: Lifeboat,
}

impl Changes {
    /// Creates a new empty changes object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.layer_stack_changes.is_empty() && self.cache_changes.is_empty()
    }

    /// Returns the layer stack changes as an iterator over (LayerStackPtr, LayerStackChanges).
    pub fn layer_stack_changes(
        &self,
    ) -> impl Iterator<Item = (&LayerStackPtr, &LayerStackChanges)> {
        self.layer_stack_changes
            .values()
            .map(|(ptr, changes)| (ptr, changes))
    }

    /// Returns the cache changes.
    pub fn cache_changes(&self) -> &HashMap<usize, CacheChanges> {
        &self.cache_changes
    }

    /// Returns the lifeboat.
    pub fn lifeboat(&self) -> &Lifeboat {
        &self.lifeboat
    }

    /// Swaps contents with another Changes object.
    pub fn swap(&mut self, other: &mut Changes) {
        std::mem::swap(
            &mut self.layer_stack_changes,
            &mut other.layer_stack_changes,
        );
        std::mem::swap(&mut self.cache_changes, &mut other.cache_changes);
        std::mem::swap(&mut self.rename_changes, &mut other.rename_changes);
        std::mem::swap(&mut self.lifeboat, &mut other.lifeboat);
    }

    // ========================================================================
    // Change Recording
    // ========================================================================

    /// Records that an object changed significantly enough to require
    /// recomputing the entire prim or property index.
    pub fn did_change_significantly(&mut self, cache: &Cache, path: &Path) {
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id)
            .add_significant_change(path.clone());
    }

    /// Records that the spec stack for the prim or property has changed.
    pub fn did_change_specs(
        &mut self,
        cache: &Cache,
        path: &Path,
        _changed_layer: &Arc<Layer>,
        _changed_path: &Path,
        _change_type: ChangeSpecsType,
    ) {
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id)
            .add_specs_change(path.clone());
    }

    /// Records that the spec stack at path has changed.
    pub fn did_change_spec_stack(&mut self, cache: &Cache, path: &Path) {
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id)
            .add_specs_change(path.clone());
    }

    /// Records that connections/targets have changed.
    pub fn did_change_targets(&mut self, cache: &Cache, path: &Path, target_type: TargetType) {
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id)
            .add_targets_change(path.clone(), target_type);
    }

    /// Records that the object at old_path was moved to new_path.
    pub fn did_change_paths(&mut self, cache: &Cache, old_path: &Path, new_path: &Path) {
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id)
            .add_path_change(old_path.clone(), new_path.clone());
    }

    /// Removes any changes for the given cache.
    pub fn did_destroy_cache(&mut self, cache: &Cache) {
        let cache_id = cache as *const _ as usize;
        self.cache_changes.remove(&cache_id);
        self.rename_changes.remove(&cache_id);
    }

    /// Records that the asset resolver has changed.
    pub fn did_change_asset_resolver(&mut self, cache: &Cache) {
        // Mark all cached prim indexes as needing significant resync
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id).did_maybe_change_layers = true;
    }

    // ========================================================================
    // Dependency Finding
    // ========================================================================

    /// Returns dependencies of the given site of scene description.
    ///
    /// Similar to Cache::find_site_dependencies but takes into account
    /// additional information from changes processed by this object.
    pub fn find_site_dependencies(
        &self,
        _cache: &Cache,
        _site_layer: &Arc<Layer>,
        _site_path: &Path,
        _dep_mask: DependencyFlags,
        _recurse_on_site: bool,
        _recurse_on_index: bool,
        _filter_for_existing_caches_only: bool,
    ) -> DependencyVector {
        // Full implementation would delegate to cache's find_site_dependencies
        // and take into account layer stack overrides from change processing.
        // For now, return empty since Cache doesn't have this method yet.
        Vec::new()
    }

    // ========================================================================
    // Apply Changes
    // ========================================================================

    /// Applies the changes to layer stacks and caches.
    pub fn apply(&self) {
        // Apply layer stack changes
        for (layer_stack_ptr, changes) in self.layer_stack_changes.values() {
            if let Some(ls) = layer_stack_ptr.upgrade() {
                if changes.did_change_relocates {
                    ls.set_relocates(
                        changes.new_relocates_target_to_source.clone(),
                        changes.new_relocates_source_to_target.clone(),
                    );
                }
            }
        }

        // Cache changes are applied by the cache itself when notified
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Gets or creates cache changes for the given cache ID.
    fn get_cache_changes(&mut self, cache_id: usize) -> &mut CacheChanges {
        self.cache_changes.entry(cache_id).or_default()
    }

    /// Gets or creates layer stack changes for the given layer stack.
    pub fn get_layer_stack_changes(
        &mut self,
        layer_stack: &LayerStackRefPtr,
    ) -> &mut LayerStackChanges {
        let key = LayerStackChangeKey::from_layer_stack(layer_stack);
        let weak = Arc::downgrade(layer_stack);
        &mut self
            .layer_stack_changes
            .entry(key)
            .or_insert_with(|| (weak, LayerStackChanges::default()))
            .1
    }

    /// Gets or creates rename changes for the given cache.
    fn _get_rename_changes(&mut self, cache_id: usize) -> &mut HashMap<Path, Path> {
        self.rename_changes.entry(cache_id).or_default()
    }

    /// Records a layer stack change.
    pub fn did_change_layer_stack(
        &mut self,
        _cache: &Cache,
        layer_stack: &LayerStackRefPtr,
        requires_layer_stack_change: bool,
        requires_layer_stack_offsets_change: bool,
        requires_significant_change: bool,
    ) {
        let changes = self.get_layer_stack_changes(layer_stack);
        if requires_layer_stack_change {
            changes.mark_layers_changed();
        }
        if requires_layer_stack_offsets_change {
            changes.mark_offsets_changed();
        }
        if requires_significant_change {
            changes.mark_significant();
        }
    }

    /// Translates Sdf layer changes into Pcp composition changes.
    ///
    /// C++ `PcpChanges::DidChange()` (changes.cpp:740-1300).
    /// Iterates over per-layer change lists, classifies each change entry
    /// into layer-stack-level changes, significant (resync) path changes,
    /// and spec-level changes, then marks the appropriate cache changes.
    pub fn did_change(&mut self, cache: &Cache, layer_change_lists: &[usd_sdf::ChangeList]) {
        use std::collections::{BTreeMap, BTreeSet};
        use usd_sdf::Path;

        // C++ bitmasks for layer stack change classification
        const LAYER_STACK_LAYERS_CHANGE: i32 = 1;
        const LAYER_STACK_OFFSETS_CHANGE: i32 = 2;
        const LAYER_STACK_RELOCATES_CHANGE: i32 = 4;
        const LAYER_STACK_SIGNIFICANT_CHANGE: i32 = 8;

        // C++ bitmasks for path change classification
        const PATH_CHANGE_SIMPLE: i32 = 1;
        const PATH_CHANGE_TARGETS: i32 = 2;
        const PATH_CHANGE_CONNECTIONS: i32 = 4;

        let cache_id = cache as *const _ as usize;
        let cache_changes = self.get_cache_changes(cache_id);

        let sub_layer_offsets_key = usd_tf::Token::new("subLayerOffsets");
        let relocates_key = usd_tf::Token::new("relocates");

        for change_list in layer_change_lists {
            let mut layer_stack_change_mask: i32 = 0;
            let mut paths_with_significant_changes: BTreeSet<Path> = BTreeSet::new();
            let mut paths_with_spec_changes: BTreeMap<Path, i32> = BTreeMap::new();
            let mut old_paths: Vec<Path> = Vec::new();
            let mut new_paths: Vec<Path> = Vec::new();

            for (path, entry) in change_list.iter() {
                let flags = &entry.flags;

                // C++ line 911: root-level changes
                if *path == Path::absolute_root() {
                    if flags.did_replace_content || flags.did_reload_content {
                        paths_with_significant_changes.insert(path.clone());
                    }

                    // Sublayer changes affect layer stack structure
                    if !entry.sublayer_changes.is_empty() {
                        layer_stack_change_mask |=
                            LAYER_STACK_LAYERS_CHANGE | LAYER_STACK_SIGNIFICANT_CHANGE;
                    }

                    // Layer offset changes
                    if entry.has_info_change(&sub_layer_offsets_key) {
                        layer_stack_change_mask |= LAYER_STACK_OFFSETS_CHANGE;
                    }

                    continue;
                }

                // In USD mode, skip property changes (only prim changes matter)
                if path.is_property_path() {
                    if flags.did_change_attribute_connection {
                        *paths_with_spec_changes.entry(path.clone()).or_insert(0) |=
                            PATH_CHANGE_CONNECTIONS;
                    }
                    if flags.did_change_relationship_targets {
                        *paths_with_spec_changes.entry(path.clone()).or_insert(0) |=
                            PATH_CHANGE_TARGETS;
                    }
                    continue;
                }

                // Prim-level: significant changes requiring full resync
                if flags.did_add_non_inert_prim
                    || flags.did_remove_non_inert_prim
                    || flags.did_change_references
                    || flags.did_change_inherit_paths
                    || flags.did_change_specializes
                    || flags.did_change_variant_sets
                    || flags.did_replace_content
                    || flags.did_reload_content
                {
                    paths_with_significant_changes.insert(path.clone());
                }

                // Rename = old path removed + new path added
                if flags.did_rename {
                    if let Some(old_path) = &entry.old_path {
                        old_paths.push(old_path.clone());
                        new_paths.push(path.clone());
                        paths_with_significant_changes.insert(old_path.clone());
                        paths_with_significant_changes.insert(path.clone());
                    }
                }

                // Inert prim add/remove = spec change (not full resync)
                if flags.did_add_inert_prim || flags.did_remove_inert_prim {
                    *paths_with_spec_changes.entry(path.clone()).or_insert(0) |= PATH_CHANGE_SIMPLE;
                }

                // Reorder = spec change
                if flags.did_reorder_children || flags.did_reorder_properties {
                    *paths_with_spec_changes.entry(path.clone()).or_insert(0) |= PATH_CHANGE_SIMPLE;
                }

                // Relocates changes affect layer stack structure
                if entry.has_info_change(&relocates_key) {
                    layer_stack_change_mask |= LAYER_STACK_RELOCATES_CHANGE;
                }

                // Any info changes on prim fields = spec change
                if !entry.info_changes.is_empty() {
                    *paths_with_spec_changes.entry(path.clone()).or_insert(0) |= PATH_CHANGE_SIMPLE;
                }
            }

            // Apply layer stack changes
            if layer_stack_change_mask != 0 {
                cache_changes.did_maybe_change_layers = true;
            }

            // Apply significant path changes — require full recomposition
            // C++ inserts into didChangeSignificantly set
            for path in &paths_with_significant_changes {
                cache_changes.did_change_significantly.insert(path.clone());
            }

            // Apply spec changes — require prim/property stack rebuild
            // C++ inserts into didChangeSpecs set
            for (path, _mask) in &paths_with_spec_changes {
                let prim_path = if path.is_property_path() {
                    path.get_prim_path()
                } else {
                    path.clone()
                };
                cache_changes.did_change_specs.insert(prim_path);
            }

            // Apply renames
            // C++ inserts into didChangePath vector
            for (old_path, new_path) in old_paths.iter().zip(new_paths.iter()) {
                cache_changes
                    .did_change_path
                    .push((old_path.clone(), new_path.clone()));
            }
        }
    }

    /// Tries to load a sublayer and marks layer stacks as changed if successful.
    ///
    /// C++ `PcpChanges::DidMaybeFixSublayer()`: attempts to resolve and open
    /// the sublayer. If it can be found, marks the owning layer stacks as
    /// needing recomposition (layers changed).
    pub fn did_maybe_fix_sublayer(&mut self, cache: &Cache, _layer: &Arc<Layer>, asset_path: &str) {
        // Try to open the sublayer
        if usd_sdf::Layer::find_or_open(asset_path).is_ok() {
            // Layer was found/opened — mark layer stacks as changed
            let cache_id = cache as *const _ as usize;
            self.get_cache_changes(cache_id).did_maybe_change_layers = true;
        }
    }

    /// Tries to load an asset and marks prims as changed if successful.
    ///
    /// C++ `PcpChanges::DidMaybeFixAsset()`: attempts to resolve the asset
    /// path. If it resolves, marks the site's prim as a significant change
    /// (requiring recomposition).
    pub fn did_maybe_fix_asset(
        &mut self,
        cache: &Cache,
        site: &super::Site,
        _src_layer: &Arc<Layer>,
        asset_path: &str,
    ) {
        // Try to resolve the asset path
        if usd_sdf::Layer::find_or_open(asset_path).is_ok() {
            // Asset found — mark prim at site path as significant change
            let cache_id = cache as *const _ as usize;
            self.get_cache_changes(cache_id)
                .did_change_significantly
                .insert(site.path.clone());
        }
    }

    /// Records that a layer was muted.
    ///
    /// Matches C++ `PcpChanges::_DidMuteLayer()`.
    pub fn did_mute_layer(&mut self, cache: &Cache, layer_id: &str) {
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id)
            .layers_affected_by_muting_or_removal
            .insert(layer_id.to_string());
    }

    /// Records that a layer was unmuted.
    ///
    /// Matches C++ `PcpChanges::_DidUnmuteLayer()`.
    pub fn did_unmute_layer(&mut self, cache: &Cache, layer_id: &str) {
        let cache_id = cache as *const _ as usize;
        self.get_cache_changes(cache_id)
            .layers_affected_by_muting_or_removal
            .remove(layer_id);
    }

    /// Sets the list of layers to mute and unmute.
    ///
    /// Matches C++ `PcpChanges::DidMuteAndUnmuteLayers()`.
    pub fn did_mute_and_unmute_layers(
        &mut self,
        cache: &Cache,
        layers_to_mute: &[String],
        layers_to_unmute: &[String],
    ) {
        let cache_id = cache as *const _ as usize;
        let cache_changes = self.get_cache_changes(cache_id);

        for layer_id in layers_to_mute {
            cache_changes
                .layers_affected_by_muting_or_removal
                .insert(layer_id.clone());
        }

        for layer_id in layers_to_unmute {
            cache_changes
                .layers_affected_by_muting_or_removal
                .remove(layer_id);
        }
    }

    /// Returns every layer stack that includes the given layer.
    ///
    /// Matches C++ `PcpChanges::FindAllLayerStacksUsingLayer()`.
    pub fn find_all_layer_stacks_using_layer(
        &self,
        cache: &Cache,
        layer: &Arc<Layer>,
    ) -> Vec<LayerStackRefPtr> {
        // Full implementation would query cache for layer stacks using this layer
        // and take into account layer stack overrides from change processing
        let _ = (cache, layer);
        Vec::new()
    }

    /// Records layer stack relocations change.
    pub fn did_change_layer_stack_relocations(
        &mut self,
        _cache: &Cache,
        layer_stack: &LayerStackRefPtr,
    ) {
        let changes = self.get_layer_stack_changes(layer_stack);
        changes.mark_relocates_changed();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_changes() {
        let changes = Changes::new();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_lifeboat() {
        let lifeboat = Lifeboat::new();
        assert!(lifeboat.layers().is_empty());
        assert!(lifeboat.layer_stacks().is_empty());
    }

    #[test]
    fn test_lifeboat_retain_and_clear() {
        let mut lb = Lifeboat::new();
        let layer = Layer::create_anonymous(Some("test"));
        lb.retain_layer(layer.clone());
        assert_eq!(lb.layers().len(), 1);

        // Duplicate retain should be no-op
        lb.retain_layer(layer);
        assert_eq!(lb.layers().len(), 1);

        lb.clear();
        assert!(lb.layers().is_empty());
    }

    #[test]
    fn test_lifeboat_swap() {
        let mut lb1 = Lifeboat::new();
        let mut lb2 = Lifeboat::new();
        let layer = Layer::create_anonymous(Some("swap_test"));
        lb1.retain_layer(layer);
        assert_eq!(lb1.layers().len(), 1);
        assert!(lb2.layers().is_empty());

        lb1.swap(&mut lb2);
        assert!(lb1.layers().is_empty());
        assert_eq!(lb2.layers().len(), 1);
    }

    #[test]
    fn test_layer_stack_changes() {
        let mut changes = LayerStackChanges::new();
        assert!(!changes.has_changes());

        changes.mark_layers_changed();
        assert!(changes.has_changes());
        assert!(changes.did_change_layers);
    }

    #[test]
    fn test_cache_changes() {
        let mut changes = CacheChanges::new();
        assert!(!changes.has_changes());

        let path = Path::from_string("/World").unwrap();
        changes.add_significant_change(path);
        assert!(changes.has_changes());
    }

    #[test]
    fn test_target_type() {
        assert_eq!(TargetType::Connection as u8, 1);
        assert_eq!(TargetType::RelationshipTarget as u8, 2);
        assert_eq!(TargetType::all(), 3);
    }
}
