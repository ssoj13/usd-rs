//! HdChangeTracker - Dirty bit constants and change tracking.
//!
//! Tracks changes from the scene delegate, providing invalidation cues to the
//! render engine. See pxr/imaging/hd/changeTracker.h for C++ reference.

use super::types::HdDirtyBits;
use crate::scene_index::base::HdSceneIndexHandle;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Rprim (renderable prim) dirty bits.
///
/// These bits track which aspects of a mesh, curve, points, or volume
/// primitive have changed and need to be re-synced.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdRprimDirtyBits;

impl HdRprimDirtyBits {
    /// Clean state - no changes.
    pub const CLEAN: HdDirtyBits = 0;

    /// Initial representation setup.
    pub const INIT_REPR: HdDirtyBits = 1 << 0;

    /// Varying state (per-frame changes; used for dirty list optimization).
    pub const VARYING: HdDirtyBits = 1 << 1;

    /// Prim ID changed.
    pub const DIRTY_PRIM_ID: HdDirtyBits = 1 << 2;

    /// Extent (bounding box) changed.
    pub const DIRTY_EXTENT: HdDirtyBits = 1 << 3;

    /// Display style changed.
    pub const DIRTY_DISPLAY_STYLE: HdDirtyBits = 1 << 4;

    /// Points/vertices changed.
    pub const DIRTY_POINTS: HdDirtyBits = 1 << 5;

    /// Primvar changed.
    pub const DIRTY_PRIMVAR: HdDirtyBits = 1 << 6;

    /// Material ID changed.
    pub const DIRTY_MATERIAL_ID: HdDirtyBits = 1 << 7;

    /// Topology (faces, connectivity) changed.
    pub const DIRTY_TOPOLOGY: HdDirtyBits = 1 << 8;

    /// Transform changed.
    pub const DIRTY_TRANSFORM: HdDirtyBits = 1 << 9;

    /// Visibility changed.
    pub const DIRTY_VISIBILITY: HdDirtyBits = 1 << 10;

    /// Normals changed.
    pub const DIRTY_NORMALS: HdDirtyBits = 1 << 11;

    /// Double-sided flag changed.
    pub const DIRTY_DOUBLE_SIDED: HdDirtyBits = 1 << 12;

    /// Cull style changed.
    pub const DIRTY_CULL_STYLE: HdDirtyBits = 1 << 13;

    /// Subdivision tags changed.
    pub const DIRTY_SUBDIV_TAGS: HdDirtyBits = 1 << 14;

    /// Widths (curves) changed.
    pub const DIRTY_WIDTHS: HdDirtyBits = 1 << 15;

    /// Instancer changed.
    pub const DIRTY_INSTANCER: HdDirtyBits = 1 << 16;

    /// Instance index changed.
    pub const DIRTY_INSTANCE_INDEX: HdDirtyBits = 1 << 17;

    /// Representation changed.
    pub const DIRTY_REPR: HdDirtyBits = 1 << 18;

    /// Render tag changed.
    pub const DIRTY_RENDER_TAG: HdDirtyBits = 1 << 19;

    /// Computation primvar descriptor changed.
    pub const DIRTY_COMPUTATION_PRIMVAR_DESC: HdDirtyBits = 1 << 20;

    /// Categories changed.
    pub const DIRTY_CATEGORIES: HdDirtyBits = 1 << 21;

    /// Volume field changed.
    pub const DIRTY_VOLUME_FIELD: HdDirtyBits = 1 << 22;

    /// All scene-related dirty bits (bits 0-22).
    pub const ALL_SCENE_DIRTY_BITS: HdDirtyBits = (1 << 23) - 1;

    /// New representation.
    pub const NEW_REPR: HdDirtyBits = 1 << 23;

    /// Start of custom bits (prims can use 24-29).
    pub const CUSTOM_BITS_BEGIN: HdDirtyBits = 1 << 24;

    /// End of custom bits.
    pub const CUSTOM_BITS_END: HdDirtyBits = 1 << 30;

    /// Mask for custom bits.
    pub const CUSTOM_BITS_MASK: HdDirtyBits = 0x7f << 24;

    /// All dirty bits: bitwise NOT of Varying.
    ///
    /// Matches C++ `AllDirty = ~Varying` exactly. All bits except bit 1
    /// (Varying) are considered "dirty". This includes bits 24-31 (custom
    /// and future bits), ensuring that any new dirty bit added later is
    /// automatically covered without updating this constant.
    pub const ALL_DIRTY: HdDirtyBits = !Self::VARYING;
}

/// Task dirty bits.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdTaskDirtyBits;

impl HdTaskDirtyBits {
    /// Clean state.
    pub const CLEAN: HdDirtyBits = 0;

    /// Task parameters changed.
    pub const DIRTY_PARAMS: HdDirtyBits = 1 << 2;

    /// Collection changed.
    pub const DIRTY_COLLECTION: HdDirtyBits = 1 << 3;

    /// Render tags changed.
    pub const DIRTY_RENDER_TAGS: HdDirtyBits = 1 << 4;

    /// All dirty (for MarkTaskDirty default).
    pub const ALL_DIRTY: HdDirtyBits =
        Self::DIRTY_PARAMS | Self::DIRTY_COLLECTION | Self::DIRTY_RENDER_TAGS;
}

/// Returns human-readable string for dirty bits (debug).
pub fn stringify_dirty_bits(dirty_bits: HdDirtyBits) -> String {
    if dirty_bits == HdRprimDirtyBits::CLEAN {
        return "Clean".to_string();
    }
    let mut parts = Vec::new();
    if (dirty_bits & HdRprimDirtyBits::VARYING) != 0 {
        parts.push("<Varying>");
    }
    if (dirty_bits & HdRprimDirtyBits::INIT_REPR) != 0 {
        parts.push("<InitRepr>");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_PRIM_ID) != 0 {
        parts.push("PrimID");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_EXTENT) != 0 {
        parts.push("Extent");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_DISPLAY_STYLE) != 0 {
        parts.push("DisplayStyle");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_POINTS) != 0 {
        parts.push("Points");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
        parts.push("Primvar");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_MATERIAL_ID) != 0 {
        parts.push("MaterialId");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_TOPOLOGY) != 0 {
        parts.push("Topology");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_TRANSFORM) != 0 {
        parts.push("Transform");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
        parts.push("Visibility");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_NORMALS) != 0 {
        parts.push("Normals");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_DOUBLE_SIDED) != 0 {
        parts.push("DoubleSided");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_CULL_STYLE) != 0 {
        parts.push("CullStyle");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_SUBDIV_TAGS) != 0 {
        parts.push("SubdivTags");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_WIDTHS) != 0 {
        parts.push("Widths");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_INSTANCER) != 0 {
        parts.push("Instancer");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_INSTANCE_INDEX) != 0 {
        parts.push("InstanceIndex");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_REPR) != 0 {
        parts.push("Repr");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_RENDER_TAG) != 0 {
        parts.push("RenderTag");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_COMPUTATION_PRIMVAR_DESC) != 0 {
        parts.push("ComputationPrimvarDesc");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_CATEGORIES) != 0 {
        parts.push("Categories");
    }
    if (dirty_bits & HdRprimDirtyBits::DIRTY_VOLUME_FIELD) != 0 {
        parts.push("VolumeField");
    }
    if (dirty_bits & HdRprimDirtyBits::NEW_REPR) != 0 {
        parts.push("NewRepr");
    }
    if (dirty_bits & !HdRprimDirtyBits::ALL_SCENE_DIRTY_BITS) != 0 {
        parts.push("CustomBits:(custom)");
    }
    parts.join(" ")
}

/// Dump dirty bits to stderr (debug).
pub fn dump_dirty_bits(dirty_bits: HdDirtyBits) {
    eprintln!("DirtyBits: {}", stringify_dirty_bits(dirty_bits));
}

/// Utility functions for dirty bit checking.
impl HdRprimDirtyBits {
    /// Returns true if any dirty flags are set (excluding Varying).
    pub fn is_dirty(bits: HdDirtyBits) -> bool {
        (bits & Self::ALL_DIRTY) != 0
    }

    /// Returns true if no dirty flags are set (except possibly Varying).
    pub fn is_clean(bits: HdDirtyBits) -> bool {
        (bits & Self::ALL_DIRTY) == 0
    }

    /// Returns true if the Varying flag is set.
    pub fn is_varying(bits: HdDirtyBits) -> bool {
        (bits & Self::VARYING) != 0
    }

    /// Returns true if extent is dirty.
    pub fn is_extent_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_EXTENT) != 0
    }

    /// Returns true if display style is dirty.
    pub fn is_display_style_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_DISPLAY_STYLE) != 0
    }

    /// Returns true if the named primvar is dirty.
    pub fn is_primvar_dirty(bits: HdDirtyBits, _id: &SdfPath, name: &Token) -> bool {
        let n = name.as_str();
        if n == "points" || n == "velocities" || n == "accelerations" || n == "nonlinearSampleCount"
        {
            (bits & Self::DIRTY_POINTS) != 0
        } else if n == "normals" {
            (bits & Self::DIRTY_NORMALS) != 0
        } else if n == "widths" {
            (bits & Self::DIRTY_WIDTHS) != 0
        } else {
            (bits & Self::DIRTY_PRIMVAR) != 0
        }
    }

    /// Returns true if any primvar is dirty (points, normals, widths, or generic).
    pub fn is_any_primvar_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits
            & (Self::DIRTY_POINTS | Self::DIRTY_NORMALS | Self::DIRTY_WIDTHS | Self::DIRTY_PRIMVAR))
            != 0
    }

    /// Returns true if topology is dirty.
    pub fn is_topology_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_TOPOLOGY) != 0
    }

    /// Returns true if double-sided flag is dirty.
    pub fn is_double_sided_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_DOUBLE_SIDED) != 0
    }

    /// Returns true if cull style is dirty.
    pub fn is_cull_style_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_CULL_STYLE) != 0
    }

    /// Returns true if subdivision tags are dirty.
    pub fn is_subdiv_tags_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_SUBDIV_TAGS) != 0
    }

    /// Returns true if transform is dirty.
    pub fn is_transform_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_TRANSFORM) != 0
    }

    /// Returns true if visibility is dirty.
    pub fn is_visibility_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_VISIBILITY) != 0
    }

    /// Returns true if prim ID is dirty.
    pub fn is_prim_id_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_PRIM_ID) != 0
    }

    /// Returns true if instancer is dirty.
    pub fn is_instancer_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_INSTANCER) != 0
    }

    /// Returns true if instance index is dirty.
    pub fn is_instance_index_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_INSTANCE_INDEX) != 0
    }

    /// Returns true if repr is dirty.
    pub fn is_repr_dirty(bits: HdDirtyBits, _id: &SdfPath) -> bool {
        (bits & Self::DIRTY_REPR) != 0
    }

    /// Set primvar dirty bit by name.
    pub fn mark_primvar_dirty(dirty_bits: &mut HdDirtyBits, name: &Token) {
        let n = name.as_str();
        let set_bits = if n == "points" {
            Self::DIRTY_POINTS
        } else if n == "normals" {
            Self::DIRTY_NORMALS
        } else if n == "widths" {
            Self::DIRTY_WIDTHS
        } else {
            Self::DIRTY_PRIMVAR
        };
        *dirty_bits |= set_bits;
    }
}

/// Change tracker for prim state (rprims, sprims, bprims, instancers, tasks).
///
/// Tracks dirty bits per prim. Used by HdRenderIndex for incremental sync.
/// Version counters are AtomicU32 (P0-5) so reads don't require &mut self.
/// Dependency maps use RwLock (P1-14) for safe concurrent access.
impl std::fmt::Debug for HdChangeTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdChangeTracker")
            .field("rprim_count", &self.rprim_state.len())
            .field("sprim_count", &self.sprim_state.len())
            .field("bprim_count", &self.bprim_state.len())
            .field(
                "scene_state_version",
                &self.scene_state_version.load(Ordering::Relaxed),
            )
            .field("has_emulation_si", &self.emulation_scene_index.is_some())
            .finish()
    }
}

/// Change tracker for prim state (rprims, sprims, bprims, instancers, tasks).
///
/// Tracks dirty bits per prim. Used by HdRenderIndex for incremental sync.
pub struct HdChangeTracker {
    rprim_state: HashMap<SdfPath, HdDirtyBits>,
    sprim_state: HashMap<SdfPath, HdDirtyBits>,
    bprim_state: HashMap<SdfPath, HdDirtyBits>,
    instancer_state: HashMap<SdfPath, HdDirtyBits>,
    task_state: HashMap<SdfPath, HdDirtyBits>,
    collection_state: HashMap<Token, i32>,
    general_state: HashMap<Token, u32>,
    /// P1-14: wrapped in RwLock so they can be read without exclusive access.
    instancer_rprim_dependencies: parking_lot::RwLock<HashMap<SdfPath, HashSet<SdfPath>>>,
    instancer_instancer_dependencies: parking_lot::RwLock<HashMap<SdfPath, HashSet<SdfPath>>>,
    instancer_sprim_dependencies: parking_lot::RwLock<HashMap<SdfPath, HashSet<SdfPath>>>,
    sprim_sprim_target_dependencies: parking_lot::RwLock<HashMap<SdfPath, HashSet<SdfPath>>>,
    sprim_sprim_source_dependencies: parking_lot::RwLock<HashMap<SdfPath, HashSet<SdfPath>>>,
    /// P0-5: atomic counters, monotonically increasing, Relaxed ordering is sufficient.
    varying_state_version: AtomicU32,
    rprim_index_version: AtomicU32,
    sprim_index_version: AtomicU32,
    bprim_index_version: AtomicU32,
    instancer_index_version: AtomicU32,
    scene_state_version: AtomicU32,
    vis_change_count: AtomicU32,
    instance_indices_change_count: AtomicU32,
    rprim_render_tag_version: AtomicU32,
    task_render_tags_version: AtomicU32,
    /// Scene index for emulation dispatch. When set, mark methods translate
    /// dirty bits to locators and dispatch through this scene index.
    emulation_scene_index: Option<HdSceneIndexHandle>,
    /// When true, the legacy scene delegate API is disabled and calls
    /// to mark methods without an emulation scene index will error.
    disable_emulation_api: bool,
}

impl HdChangeTracker {
    /// Create a new change tracker.
    pub fn new() -> Self {
        Self {
            rprim_state: HashMap::new(),
            sprim_state: HashMap::new(),
            bprim_state: HashMap::new(),
            instancer_state: HashMap::new(),
            task_state: HashMap::new(),
            collection_state: HashMap::new(),
            general_state: HashMap::new(),
            instancer_rprim_dependencies: parking_lot::RwLock::new(HashMap::new()),
            instancer_instancer_dependencies: parking_lot::RwLock::new(HashMap::new()),
            instancer_sprim_dependencies: parking_lot::RwLock::new(HashMap::new()),
            sprim_sprim_target_dependencies: parking_lot::RwLock::new(HashMap::new()),
            sprim_sprim_source_dependencies: parking_lot::RwLock::new(HashMap::new()),
            varying_state_version: AtomicU32::new(1),
            rprim_index_version: AtomicU32::new(1),
            sprim_index_version: AtomicU32::new(1),
            bprim_index_version: AtomicU32::new(1),
            instancer_index_version: AtomicU32::new(1),
            scene_state_version: AtomicU32::new(1),
            vis_change_count: AtomicU32::new(1),
            instance_indices_change_count: AtomicU32::new(1),
            rprim_render_tag_version: AtomicU32::new(1),
            task_render_tags_version: AtomicU32::new(1),
            emulation_scene_index: None,
            disable_emulation_api: false,
        }
    }

    fn add_dependency(
        dep_map: &parking_lot::RwLock<HashMap<SdfPath, HashSet<SdfPath>>>,
        parent: SdfPath,
        child: SdfPath,
    ) {
        dep_map.write().entry(parent).or_default().insert(child);
    }

    fn remove_dependency(
        dep_map: &parking_lot::RwLock<HashMap<SdfPath, HashSet<SdfPath>>>,
        parent: &SdfPath,
        child: &SdfPath,
    ) {
        let mut map = dep_map.write();
        if let Some(children) = map.get_mut(parent) {
            children.remove(child);
            if children.is_empty() {
                map.remove(parent);
            }
        }
    }

    /// Add named state for tracking.
    pub fn add_state(&mut self, name: &Token) {
        self.general_state.insert(name.clone(), 1);
    }

    /// Mark named state dirty (bump version).
    pub fn mark_state_dirty(&mut self, name: &Token) {
        if let Some(v) = self.general_state.get_mut(name) {
            *v = v.wrapping_add(1);
        }
    }

    /// Get version of named state.
    pub fn get_state_version(&self, name: &Token) -> u32 {
        self.general_state.get(name).copied().unwrap_or(0)
    }

    /// Add instancer->rprim dependency. When instancer changes, rprim gets DirtyInstancer.
    pub fn add_instancer_rprim_dependency(&mut self, instancer_id: &SdfPath, rprim_id: &SdfPath) {
        Self::add_dependency(
            &self.instancer_rprim_dependencies,
            instancer_id.clone(),
            rprim_id.clone(),
        );
    }

    /// Remove instancer->rprim dependency.
    pub fn remove_instancer_rprim_dependency(
        &mut self,
        instancer_id: &SdfPath,
        rprim_id: &SdfPath,
    ) {
        Self::remove_dependency(&self.instancer_rprim_dependencies, instancer_id, rprim_id);
    }

    /// Add parent instancer->child instancer dependency.
    pub fn add_instancer_instancer_dependency(
        &mut self,
        parent_instancer_id: &SdfPath,
        instancer_id: &SdfPath,
    ) {
        Self::add_dependency(
            &self.instancer_instancer_dependencies,
            parent_instancer_id.clone(),
            instancer_id.clone(),
        );
    }

    /// Remove parent instancer->child instancer dependency.
    pub fn remove_instancer_instancer_dependency(
        &mut self,
        parent_instancer_id: &SdfPath,
        instancer_id: &SdfPath,
    ) {
        Self::remove_dependency(
            &self.instancer_instancer_dependencies,
            parent_instancer_id,
            instancer_id,
        );
    }

    /// Add instancer->sprim dependency.
    pub fn add_instancer_sprim_dependency(&mut self, instancer_id: &SdfPath, sprim_id: &SdfPath) {
        Self::add_dependency(
            &self.instancer_sprim_dependencies,
            instancer_id.clone(),
            sprim_id.clone(),
        );
    }

    /// Remove instancer->sprim dependency.
    pub fn remove_instancer_sprim_dependency(
        &mut self,
        instancer_id: &SdfPath,
        sprim_id: &SdfPath,
    ) {
        Self::remove_dependency(&self.instancer_sprim_dependencies, instancer_id, sprim_id);
    }

    /// Add parent sprim->child sprim dependency.
    pub fn add_sprim_sprim_dependency(&mut self, parent_sprim_id: &SdfPath, sprim_id: &SdfPath) {
        Self::add_dependency(
            &self.sprim_sprim_target_dependencies,
            parent_sprim_id.clone(),
            sprim_id.clone(),
        );
        Self::add_dependency(
            &self.sprim_sprim_source_dependencies,
            sprim_id.clone(),
            parent_sprim_id.clone(),
        );
    }

    /// Remove parent sprim->child sprim dependency.
    pub fn remove_sprim_sprim_dependency(&mut self, parent_sprim_id: &SdfPath, sprim_id: &SdfPath) {
        Self::remove_dependency(
            &self.sprim_sprim_target_dependencies,
            parent_sprim_id,
            sprim_id,
        );
        Self::remove_dependency(
            &self.sprim_sprim_source_dependencies,
            sprim_id,
            parent_sprim_id,
        );
    }

    /// Remove all dependencies involving sprim_id (as parent or child).
    pub fn remove_sprim_from_sprim_sprim_dependencies(&mut self, sprim_id: &SdfPath) {
        let is_empty = self.sprim_sprim_target_dependencies.read().is_empty();
        if is_empty {
            return;
        }
        // Collect children under write lock, then release before marking dirty (avoids deadlock).
        let children: Vec<SdfPath> = {
            let mut map = self.sprim_sprim_target_dependencies.write();
            map.remove(sprim_id)
                .map(|s| s.into_iter().collect())
                .unwrap_or_default()
        };
        for child_path in children {
            // Full invalidation when a parent sprim is removed is correct here.
            self.mark_sprim_dirty(&child_path, HdRprimDirtyBits::ALL_DIRTY);
            Self::remove_dependency(&self.sprim_sprim_source_dependencies, &child_path, sprim_id);
        }
        let parents: Vec<SdfPath> = {
            let mut map = self.sprim_sprim_source_dependencies.write();
            map.remove(sprim_id)
                .map(|s| s.into_iter().collect())
                .unwrap_or_default()
        };
        for parent_path in parents {
            Self::remove_dependency(
                &self.sprim_sprim_target_dependencies,
                &parent_path,
                sprim_id,
            );
        }
    }

    // ----------------------------------------------------------------------- //
    // Emulation scene index
    // ----------------------------------------------------------------------- //

    /// Set the emulation scene index for dirty bit dispatch.
    pub fn set_emulation_scene_index(&mut self, si: HdSceneIndexHandle) {
        self.emulation_scene_index = Some(si);
    }

    /// Get the emulation scene index, if set.
    pub fn get_emulation_scene_index(&self) -> Option<&HdSceneIndexHandle> {
        self.emulation_scene_index.as_ref()
    }

    /// Clear the emulation scene index.
    pub fn clear_emulation_scene_index(&mut self) {
        self.emulation_scene_index = None;
    }

    /// Set whether the legacy emulation API is disabled.
    pub fn set_disable_emulation_api(&mut self, disable: bool) {
        self.disable_emulation_api = disable;
    }

    /// Returns true if the legacy emulation API is disabled.
    pub fn is_emulation_api_disabled(&self) -> bool {
        self.disable_emulation_api
    }

    // ----------------------------------------------------------------------- //
    // Rprim tracking
    // ----------------------------------------------------------------------- //

    /// Start tracking rprim.
    pub fn rprim_inserted(&mut self, id: &SdfPath, initial_dirty_state: HdDirtyBits) {
        self.rprim_state.insert(id.clone(), initial_dirty_state);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.rprim_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Stop tracking rprim.
    pub fn rprim_removed(&mut self, id: &SdfPath) {
        self.rprim_state.remove(id);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.rprim_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get rprim dirty bits.
    pub fn get_rprim_dirty_bits(&self, id: &SdfPath) -> HdDirtyBits {
        self.rprim_state.get(id).copied().unwrap_or(0)
    }

    /// Mark rprim dirty. Matches C++ _MarkRprimDirty logic including:
    /// - Early-out when no new bits (except DirtyRenderTag/DirtyRepr)
    /// - InitRepr special handling (no scene state bump)
    /// - rprim_index_version bump for RenderTag/Repr changes
    pub fn mark_rprim_dirty(&mut self, id: &SdfPath, mut bits: HdDirtyBits) {
        if bits == HdRprimDirtyBits::CLEAN {
            return;
        }

        // C++ parity: DirtyPrimvar implies DirtyPoints/Normals/Widths
        if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
            bits |= HdRprimDirtyBits::DIRTY_POINTS
                | HdRprimDirtyBits::DIRTY_NORMALS
                | HdRprimDirtyBits::DIRTY_WIDTHS;
        }

        if let Some(old_bits) = self.rprim_state.get_mut(id) {
            // Early out if no new bits, unless RenderTag/Repr which always need processing
            if (bits & !*old_bits) == 0 {
                if (bits & (HdRprimDirtyBits::DIRTY_RENDER_TAG | HdRprimDirtyBits::DIRTY_REPR)) == 0
                {
                    return;
                }
            }

            // InitRepr: just set the bit without touching scene state version
            if bits == HdRprimDirtyBits::INIT_REPR {
                *old_bits |= HdRprimDirtyBits::INIT_REPR;
                return;
            }

            // Set Varying bit if not already set
            if (*old_bits & HdRprimDirtyBits::VARYING) == 0 {
                bits |= HdRprimDirtyBits::VARYING;
                self.varying_state_version.fetch_add(1, Ordering::Relaxed);
            }
            *old_bits |= bits;
            self.scene_state_version.fetch_add(1, Ordering::Relaxed);

            if (bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
                self.vis_change_count.fetch_add(1, Ordering::Relaxed);
            }
            if (bits & HdRprimDirtyBits::DIRTY_INSTANCE_INDEX) != 0 {
                self.instance_indices_change_count
                    .fetch_add(1, Ordering::Relaxed);
            }
            if (bits & HdRprimDirtyBits::DIRTY_RENDER_TAG) != 0 {
                self.rprim_render_tag_version
                    .fetch_add(1, Ordering::Relaxed);
            }
            // RenderTag/Repr affect dirty lists and batching - treat as scene edit
            if (bits & (HdRprimDirtyBits::DIRTY_RENDER_TAG | HdRprimDirtyBits::DIRTY_REPR)) != 0 {
                self.rprim_index_version.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Mark rprim clean. Preserves Varying bit per C++.
    pub fn mark_rprim_clean(&mut self, id: &SdfPath, new_bits: HdDirtyBits) {
        if let Some(bits) = self.rprim_state.get_mut(id) {
            *bits = (*bits & HdRprimDirtyBits::VARYING) | new_bits;
        }
    }

    /// Reset varying state on all clean prims.
    pub fn reset_varying_state(&mut self) {
        self.varying_state_version.fetch_add(1, Ordering::Relaxed);
        for bits in self.rprim_state.values_mut() {
            if HdRprimDirtyBits::is_clean(*bits) {
                *bits &= !HdRprimDirtyBits::VARYING;
            }
        }
    }

    // ----------------------------------------------------------------------- //
    // Instancer tracking
    // ----------------------------------------------------------------------- //

    /// Start tracking instancer.
    pub fn instancer_inserted(&mut self, id: &SdfPath, initial_dirty_state: HdDirtyBits) {
        self.instancer_state.insert(id.clone(), initial_dirty_state);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.instancer_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Stop tracking instancer.
    pub fn instancer_removed(&mut self, id: &SdfPath) {
        self.instancer_state.remove(id);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.instancer_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get instancer dirty bits.
    pub fn get_instancer_dirty_bits(&self, id: &SdfPath) -> HdDirtyBits {
        self.instancer_state.get(id).copied().unwrap_or(0)
    }

    /// Mark instancer dirty. Propagates to dependent rprims, instancers, sprims.
    pub fn mark_instancer_dirty(&mut self, id: &SdfPath, bits: HdDirtyBits) {
        if bits == HdRprimDirtyBits::CLEAN {
            return;
        }
        if let Some(old_bits) = self.instancer_state.get_mut(id) {
            if (bits & !*old_bits) == 0 {
                return;
            }
            *old_bits |= bits;
            self.scene_state_version.fetch_add(1, Ordering::Relaxed);

            let to_propagate = HdRprimDirtyBits::DIRTY_INSTANCER
                | if (bits & HdRprimDirtyBits::DIRTY_TRANSFORM) != 0 {
                    HdRprimDirtyBits::DIRTY_TRANSFORM
                } else {
                    0
                }
                | if (bits
                    & (HdRprimDirtyBits::DIRTY_INSTANCE_INDEX | HdRprimDirtyBits::DIRTY_VISIBILITY))
                    != 0
                {
                    self.instance_indices_change_count
                        .fetch_add(1, Ordering::Relaxed);
                    HdRprimDirtyBits::DIRTY_INSTANCE_INDEX
                } else {
                    0
                };

            // Collect all deps before recursing to avoid holding the lock during recursion
            let ii_deps: Vec<SdfPath> = self
                .instancer_instancer_dependencies
                .read()
                .get(id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            let ir_deps: Vec<SdfPath> = self
                .instancer_rprim_dependencies
                .read()
                .get(id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            let is_deps: Vec<SdfPath> = self
                .instancer_sprim_dependencies
                .read()
                .get(id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();

            for dep in ii_deps {
                self.mark_instancer_dirty(&dep, to_propagate);
            }
            for dep in ir_deps {
                self.mark_rprim_dirty(&dep, to_propagate);
            }
            for dep in is_deps {
                self.mark_sprim_dirty(&dep, to_propagate);
            }
        }
    }

    /// Mark instancer clean. Preserves Varying bit.
    pub fn mark_instancer_clean(&mut self, id: &SdfPath, new_bits: HdDirtyBits) {
        if let Some(bits) = self.instancer_state.get_mut(id) {
            *bits = (*bits & HdRprimDirtyBits::VARYING) | new_bits;
        }
    }

    // ----------------------------------------------------------------------- //
    // Task tracking
    // ----------------------------------------------------------------------- //

    /// Start tracking task.
    pub fn task_inserted(&mut self, id: &SdfPath, initial_dirty_state: HdDirtyBits) {
        self.task_state.insert(id.clone(), initial_dirty_state);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Stop tracking task.
    pub fn task_removed(&mut self, id: &SdfPath) {
        self.task_state.remove(id);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get task dirty bits.
    pub fn get_task_dirty_bits(&self, id: &SdfPath) -> HdDirtyBits {
        self.task_state.get(id).copied().unwrap_or(0)
    }

    /// Mark task dirty. Bumps task_render_tags_version when DirtyRenderTags is newly set.
    pub fn mark_task_dirty(&mut self, id: &SdfPath, bits: HdDirtyBits) {
        if bits == HdRprimDirtyBits::CLEAN {
            return;
        }
        if let Some(old_bits) = self.task_state.get_mut(id) {
            // Bump render tags version when DirtyRenderTags is newly set
            if (bits & HdTaskDirtyBits::DIRTY_RENDER_TAGS) != 0
                && (*old_bits & HdTaskDirtyBits::DIRTY_RENDER_TAGS) == 0
            {
                self.task_render_tags_version
                    .fetch_add(1, Ordering::Relaxed);
            }
            *old_bits |= bits;
            self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Mark task clean. Matches C++ MarkTaskClean: sets bits to new_bits.
    pub fn mark_task_clean(&mut self, id: &SdfPath, new_bits: HdDirtyBits) {
        if let Some(bits) = self.task_state.get_mut(id) {
            *bits = new_bits;
        }
    }

    /// Get task render tags version.
    pub fn get_task_render_tags_version(&self) -> u32 {
        self.task_render_tags_version.load(Ordering::Relaxed)
    }

    // ----------------------------------------------------------------------- //
    // Collection tracking
    // ----------------------------------------------------------------------- //

    /// Add collection for tracking.
    pub fn add_collection(&mut self, name: &Token) {
        self.collection_state.insert(name.clone(), 1);
    }

    /// Mark collection dirty (bump version).
    pub fn mark_collection_dirty(&mut self, name: &Token) {
        if let Some(v) = self.collection_state.get_mut(name) {
            *v = v.wrapping_add(1);
        }
        // C++ changeTracker.cpp:1125 - bump scene state version
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get collection version. Includes rprim_index_version so collections
    /// are invalidated when the rprim index changes (adds/removes/render tag).
    pub fn get_collection_version(&self, name: &Token) -> u32 {
        let coll = self.collection_state.get(name).copied().unwrap_or(0) as u32;
        coll.wrapping_add(self.rprim_index_version.load(Ordering::Relaxed))
    }

    /// Check if rprim is dirty.
    pub fn is_rprim_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_dirty(self.get_rprim_dirty_bits(id))
    }

    /// Check if extent is dirty.
    pub fn is_extent_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_extent_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if display style is dirty.
    pub fn is_display_style_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_display_style_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if primvar is dirty.
    pub fn is_primvar_dirty(&self, id: &SdfPath, name: &Token) -> bool {
        HdRprimDirtyBits::is_primvar_dirty(self.get_rprim_dirty_bits(id), id, name)
    }

    /// Check if any primvar is dirty.
    pub fn is_any_primvar_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_any_primvar_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if topology is dirty.
    pub fn is_topology_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_topology_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if double-sided is dirty.
    pub fn is_double_sided_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_double_sided_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if cull style is dirty.
    pub fn is_cull_style_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_cull_style_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if subdiv tags are dirty.
    pub fn is_subdiv_tags_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_subdiv_tags_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if transform is dirty.
    pub fn is_transform_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_transform_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if visibility is dirty.
    pub fn is_visibility_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_visibility_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if prim id is dirty.
    pub fn is_prim_id_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_prim_id_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if instancer is dirty.
    pub fn is_instancer_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_instancer_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if instance index is dirty.
    pub fn is_instance_index_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_instance_index_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Check if repr is dirty.
    pub fn is_repr_dirty(&self, id: &SdfPath) -> bool {
        HdRprimDirtyBits::is_repr_dirty(self.get_rprim_dirty_bits(id), id)
    }

    /// Reset varying state on one rprim. Unconditionally clears Varying bit.
    /// Used for invisible prims where dirty bits are NOT cleaned during sync.
    pub fn reset_rprim_varying_state(&mut self, id: &SdfPath) {
        if let Some(bits) = self.rprim_state.get_mut(id) {
            *bits &= !HdRprimDirtyBits::VARYING;
        }
    }

    /// Mark all rprims dirty. Matches C++ MarkAllRprimsDirty with per-prim
    /// varying state logic and RenderTag/Repr exception handling.
    pub fn mark_all_rprims_dirty(&mut self, bits: HdDirtyBits) {
        if bits == HdRprimDirtyBits::CLEAN {
            return;
        }

        let mut varying_state_updated = false;

        for rprim_bits in self.rprim_state.values_mut() {
            // If RenderTag/Repr dirty, always update even if bits already set
            if (bits
                & ((!*rprim_bits)
                    | HdRprimDirtyBits::DIRTY_RENDER_TAG
                    | HdRprimDirtyBits::DIRTY_REPR))
                != 0
            {
                *rprim_bits |= bits;

                if (*rprim_bits & HdRprimDirtyBits::VARYING) == 0 {
                    *rprim_bits |= HdRprimDirtyBits::VARYING;
                    varying_state_updated = true;
                }
            }
        }

        if varying_state_updated {
            self.varying_state_version.fetch_add(1, Ordering::Relaxed);
        }

        // Version bumps happen unconditionally
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        if (bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
            self.vis_change_count.fetch_add(1, Ordering::Relaxed);
        }
        if (bits & HdRprimDirtyBits::DIRTY_INSTANCE_INDEX) != 0 {
            self.instance_indices_change_count
                .fetch_add(1, Ordering::Relaxed);
        }
        if (bits & HdRprimDirtyBits::DIRTY_RENDER_TAG) != 0 {
            self.rprim_render_tag_version
                .fetch_add(1, Ordering::Relaxed);
        }
        if (bits & (HdRprimDirtyBits::DIRTY_RENDER_TAG | HdRprimDirtyBits::DIRTY_REPR)) != 0 {
            self.rprim_index_version.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Mark primvar dirty. Only marks existing tracked rprims (C++ parity:
    /// does not create phantom entries for untracked prims).
    pub fn mark_primvar_dirty(&mut self, id: &SdfPath, name: &Token) {
        if let Some(bits) = self.rprim_state.get_mut(id) {
            HdRprimDirtyBits::mark_primvar_dirty(bits, name);
            self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        }
    }

    // ----------------------------------------------------------------------- //
    // Sprim tracking
    // ----------------------------------------------------------------------- //

    /// Start tracking sprim.
    pub fn sprim_inserted(&mut self, id: &SdfPath, initial_dirty_state: HdDirtyBits) {
        self.sprim_state.insert(id.clone(), initial_dirty_state);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.sprim_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Stop tracking sprim.
    pub fn sprim_removed(&mut self, id: &SdfPath) {
        self.sprim_state.remove(id);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.sprim_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get sprim dirty bits.
    pub fn get_sprim_dirty_bits(&self, id: &SdfPath) -> HdDirtyBits {
        self.sprim_state.get(id).copied().unwrap_or(0)
    }

    /// Mark sprim dirty. Propagates only the originally-set bits to dependent sprims (P1-12).
    pub fn mark_sprim_dirty(&mut self, id: &SdfPath, bits: HdDirtyBits) {
        if bits == HdRprimDirtyBits::CLEAN {
            return;
        }
        if let Some(old_bits) = self.sprim_state.get_mut(id) {
            *old_bits |= bits;
            self.scene_state_version.fetch_add(1, Ordering::Relaxed);

            // P1-12: propagate the same bits (not !CLEAN which would set undefined bits).
            let ss_deps: Vec<SdfPath> = self
                .sprim_sprim_target_dependencies
                .read()
                .get(id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            for dep in ss_deps {
                self.mark_sprim_dirty(&dep, bits);
            }
        }
    }

    /// Mark sprim clean.
    pub fn mark_sprim_clean(&mut self, id: &SdfPath, new_bits: HdDirtyBits) {
        if let Some(bits) = self.sprim_state.get_mut(id) {
            *bits = new_bits;
        }
    }

    // ----------------------------------------------------------------------- //
    // Bprim tracking
    // ----------------------------------------------------------------------- //

    /// Start tracking bprim.
    pub fn bprim_inserted(&mut self, id: &SdfPath, initial_dirty_state: HdDirtyBits) {
        self.bprim_state.insert(id.clone(), initial_dirty_state);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.bprim_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Stop tracking bprim.
    pub fn bprim_removed(&mut self, id: &SdfPath) {
        self.bprim_state.remove(id);
        self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        self.bprim_index_version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get bprim dirty bits.
    pub fn get_bprim_dirty_bits(&self, id: &SdfPath) -> HdDirtyBits {
        self.bprim_state.get(id).copied().unwrap_or(0)
    }

    /// Mark bprim dirty.
    pub fn mark_bprim_dirty(&mut self, id: &SdfPath, bits: HdDirtyBits) {
        if let Some(old_bits) = self.bprim_state.get_mut(id) {
            *old_bits |= bits;
            self.scene_state_version.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Mark bprim clean.
    pub fn mark_bprim_clean(&mut self, id: &SdfPath, new_bits: HdDirtyBits) {
        if let Some(bits) = self.bprim_state.get_mut(id) {
            *bits = new_bits;
        }
    }

    // ----------------------------------------------------------------------- //
    // Version accessors
    // ----------------------------------------------------------------------- //

    /// Get varying state version.
    pub fn get_varying_state_version(&self) -> u32 {
        self.varying_state_version.load(Ordering::Relaxed)
    }

    /// Get rprim index version.
    pub fn get_rprim_index_version(&self) -> u32 {
        self.rprim_index_version.load(Ordering::Relaxed)
    }

    /// Get sprim index version.
    pub fn get_sprim_index_version(&self) -> u32 {
        self.sprim_index_version.load(Ordering::Relaxed)
    }

    /// Get bprim index version.
    pub fn get_bprim_index_version(&self) -> u32 {
        self.bprim_index_version.load(Ordering::Relaxed)
    }

    /// Get instancer index version.
    pub fn get_instancer_index_version(&self) -> u32 {
        self.instancer_index_version.load(Ordering::Relaxed)
    }

    /// Get scene state version.
    pub fn get_scene_state_version(&self) -> u32 {
        self.scene_state_version.load(Ordering::Relaxed)
    }

    /// Get visibility change count.
    pub fn get_visibility_change_count(&self) -> u32 {
        self.vis_change_count.load(Ordering::Relaxed)
    }

    /// Get instance indices change count.
    pub fn get_instance_indices_change_count(&self) -> u32 {
        self.instance_indices_change_count.load(Ordering::Relaxed)
    }

    /// Get rprim render tag version.
    pub fn get_render_tag_version(&self) -> u32 {
        self.rprim_render_tag_version.load(Ordering::Relaxed)
    }
}
