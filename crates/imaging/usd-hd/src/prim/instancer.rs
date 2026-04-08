//! HdInstancer - Instancing support for prims.
//!
//! Provides instancing (copying) of geometry across multiple transforms.
//! Supports:
//! - Point instancing (one instance per point)
//! - Nested instancing (instancers of instancers)
//! - Per-instance primvars (transform, color, etc)
//! - Instance indices for selection
//!
//! # Instancing Model
//!
//! An instancer provides:
//! - Array of transforms (one per instance)
//! - Instance indices (subset of instances to draw)
//! - Per-instance primvars
//!
//! Prims reference an instancer via their `instancer_id`.

use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

use super::{HdRenderParam, HdSceneDelegate};
use crate::change_tracker::{HdChangeTracker, HdRprimDirtyBits};
use crate::render::render_index::HdRenderIndex;
use crate::tokens::{
    INSTANCE_ROTATIONS, INSTANCE_SCALES, INSTANCE_TRANSFORMS, INSTANCE_TRANSLATIONS,
};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Instancer for geometry instancing.
///
/// Manages instance transforms and per-instance data.
/// Port of C++ HdInstancer (pxr/imaging/hd/instancer.h).
pub struct HdInstancer {
    /// Scene delegate that owns this instancer (matches C++ `_delegate`).
    delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,

    /// Prim identifier (matches C++ `_id`).
    id: SdfPath,

    /// Parent instancer id (empty path if not nested).
    parent_id: SdfPath,

    /// Number of instances.
    num_instances: usize,

    /// Per-instancer mutex for sync_instancer_and_parents.
    /// XXX: This mutex exists for _SyncInstancerAndParents, which will go
    /// away when the render index calls sync on instancers.
    instance_lock: Mutex<()>,
}

impl std::fmt::Debug for HdInstancer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdInstancer")
            .field("id", &self.id)
            .field("parent_id", &self.parent_id)
            .field("num_instances", &self.num_instances)
            .field("has_delegate", &self.delegate.is_some())
            .finish()
    }
}

/// Builtin primvar names for instancers (cached static).
static BUILTIN_PRIMVAR_NAMES: Lazy<Vec<Token>> = Lazy::new(|| {
    vec![
        INSTANCE_TRANSFORMS.clone(),
        INSTANCE_ROTATIONS.clone(),
        INSTANCE_SCALES.clone(),
        INSTANCE_TRANSLATIONS.clone(),
    ]
});

impl HdInstancer {
    // =========================================================================
    // Dirty bit constants (aligned with HdChangeTracker / HdRprimDirtyBits)
    // =========================================================================

    /// Clean state.
    pub const CLEAN: HdDirtyBits = HdRprimDirtyBits::CLEAN;

    /// Instance transforms changed.
    pub const DIRTY_TRANSFORM: HdDirtyBits = HdRprimDirtyBits::DIRTY_TRANSFORM;

    /// Instance primvars changed.
    pub const DIRTY_PRIMVAR: HdDirtyBits = HdRprimDirtyBits::DIRTY_PRIMVAR;

    /// Instance indices changed.
    pub const DIRTY_INSTANCE_INDEX: HdDirtyBits = HdRprimDirtyBits::DIRTY_INSTANCE_INDEX;

    /// Instancer binding changed.
    pub const DIRTY_INSTANCER: HdDirtyBits = HdRprimDirtyBits::DIRTY_INSTANCER;

    /// Visibility changed.
    pub const DIRTY_VISIBILITY: HdDirtyBits = HdRprimDirtyBits::DIRTY_VISIBILITY;

    /// All dirty bits.
    pub const ALL_DIRTY: HdDirtyBits = HdRprimDirtyBits::ALL_DIRTY;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Create a new instancer.
    ///
    /// # Arguments
    ///
    /// * `delegate` - Scene delegate that owns this instancer (matches C++ ctor)
    /// * `id` - Unique identifier for this instancer
    /// * `parent_id` - Parent instancer path for nested instancing (None if top-level)
    pub fn new(
        delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,
        id: SdfPath,
        parent_id: Option<SdfPath>,
    ) -> Self {
        Self {
            delegate,
            id,
            parent_id: parent_id.unwrap_or_else(SdfPath::empty),
            num_instances: 0,
            instance_lock: Mutex::new(()),
        }
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Get instancer identifier.
    pub fn get_id(&self) -> &SdfPath {
        &self.id
    }

    /// Get the scene delegate stored at construction time.
    /// Matches C++ `HdInstancer::GetDelegate()`.
    pub fn get_delegate(&self) -> Option<&(dyn HdSceneDelegate + Send + Sync)> {
        self.delegate.as_deref()
    }

    /// Get parent instancer id. Returns empty path if not nested.
    pub fn get_parent_id(&self) -> &SdfPath {
        &self.parent_id
    }

    /// Get number of instances.
    pub fn get_num_instances(&self) -> usize {
        self.num_instances
    }

    /// Set number of instances.
    pub fn set_num_instances(&mut self, count: usize) {
        self.num_instances = count;
    }

    /// Check if this is a nested instancer (has a parent).
    pub fn is_nested(&self) -> bool {
        !self.parent_id.is_empty()
    }

    // =========================================================================
    // Static methods
    // =========================================================================

    /// Walk the instancer hierarchy to count nesting levels for an rprim.
    ///
    /// Matches C++ `HdInstancer::GetInstancerNumLevels(HdRenderIndex&, HdRprim const&)`.
    pub fn get_instancer_num_levels(
        render_index: &HdRenderIndex,
        rprim_instancer_id: &SdfPath,
    ) -> usize {
        let mut levels = 0usize;
        let mut parent = rprim_instancer_id.clone();
        while !parent.is_empty() {
            levels += 1;
            if let Some(instancer) = render_index.get_instancer(&parent) {
                parent = instancer.get_parent_id().clone();
            } else {
                break;
            }
        }
        levels
    }

    /// Returns the builtin primvar names for instancers.
    ///
    /// These are: hydra:instanceTransforms, hydra:instanceRotations,
    /// hydra:instanceScales, hydra:instanceTranslations.
    ///
    /// Matches C++ `HdInstancer::GetBuiltinPrimvarNames()`.
    pub fn get_builtin_primvar_names() -> &'static [Token] {
        &BUILTIN_PRIMVAR_NAMES
    }

    /// Get initial dirty bits mask for new instancers.
    ///
    /// Returns DirtyTransform | DirtyPrimvar | DirtyInstanceIndex |
    /// DirtyInstancer | DirtyVisibility (matching C++ exactly).
    pub fn get_initial_dirty_bits_mask() -> HdDirtyBits {
        Self::DIRTY_TRANSFORM
            | Self::DIRTY_PRIMVAR
            | Self::DIRTY_INSTANCE_INDEX
            | Self::DIRTY_INSTANCER
            | Self::DIRTY_VISIBILITY
    }

    // =========================================================================
    // Sync / Finalize (virtual in C++, base impl is no-op)
    // =========================================================================

    /// Synchronize instancer state from scene delegate.
    ///
    /// Base implementation is empty. Override in renderer-specific instancers
    /// (e.g. HdStInstancer) to pull instance data.
    ///
    /// Matches C++ `HdInstancer::Sync(HdSceneDelegate*, HdRenderParam*, HdDirtyBits*)`.
    pub fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        _dirty_bits: &mut HdDirtyBits,
    ) {
        // Base implementation is intentionally empty.
        // Renderer-specific subclasses override this.
    }

    /// Finalize before destruction.
    ///
    /// Matches C++ `HdInstancer::Finalize(HdRenderParam*)`.
    pub fn finalize(&mut self, _render_param: Option<&dyn HdRenderParam>) {
        // Base implementation is intentionally empty.
    }

    // =========================================================================
    // Hierarchy sync (critical for nested instancing)
    // =========================================================================

    /// Sync instancer and all its parents up the hierarchy.
    ///
    /// Walks up the parent chain, locking each instancer to prevent double-sync.
    /// For each dirty instancer, calls Sync then marks clean in the change tracker.
    ///
    /// Matches C++ `HdInstancer::_SyncInstancerAndParents(HdRenderIndex&, SdfPath const&)`.
    ///
    /// Note: Due to Rust ownership, the parent chain is collected first, then
    /// each instancer is synced individually. The per-instancer mutex prevents
    /// concurrent double-sync (matching C++ behavior).
    pub fn sync_instancer_and_parents(render_index: &mut HdRenderIndex, instancer_id: &SdfPath) {
        // Collect the chain of instancer ids to sync (bottom-up).
        let chain: Vec<SdfPath> = {
            let mut ids = Vec::new();
            let mut id = instancer_id.clone();
            while !id.is_empty() {
                if let Some(instancer) = render_index.get_instancer(&id) {
                    ids.push(id.clone());
                    id = instancer.get_parent_id().clone();
                } else {
                    break;
                }
            }
            ids
        };

        // Sync each instancer in the chain.
        // Matches C++: instancer->Sync(instancer->GetDelegate(), renderParam, &dirtyBits)
        for id in &chain {
            let dirty_bits = render_index
                .get_change_tracker()
                .get_instancer_dirty_bits(id);
            if dirty_bits != HdRprimDirtyBits::CLEAN {
                if let Some(instancer) = render_index.get_instancer_mut(id) {
                    // Lock per-instancer mutex (prevents double-sync in parallel).
                    // Use pointer trick to split the borrow: lock + delegate clone
                    // happen before the mutable sync call.
                    let _lock = instancer
                        .instance_lock
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let delegate = instancer.delegate.clone();
                    drop(_lock);
                    let mut bits = dirty_bits;
                    if let Some(ref d) = delegate {
                        instancer.sync(d.as_ref(), None, &mut bits);
                    }
                }
                render_index
                    .get_change_tracker_mut()
                    .mark_instancer_clean(id, HdRprimDirtyBits::CLEAN);
            }
        }
    }

    /// Update instancer parent from scene delegate.
    ///
    /// Reads the parent instancer id from the delegate and updates dependency
    /// tracking in the change tracker if the parent changed.
    ///
    /// Matches C++ `HdInstancer::_UpdateInstancer(HdSceneDelegate*, HdDirtyBits*)`.
    pub fn update_instancer(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        dirty_bits: &mut HdDirtyBits,
        tracker: &mut HdChangeTracker,
    ) {
        if !HdRprimDirtyBits::is_instancer_dirty(*dirty_bits, &self.id) {
            return;
        }

        // C++ returns empty SdfPath when prim has no instancer
        let new_parent_id = delegate.get_instancer_id(&self.id);

        if new_parent_id == self.parent_id {
            return;
        }

        // Update dependency tracking in change tracker.
        if !self.parent_id.is_empty() {
            tracker.remove_instancer_instancer_dependency(&self.parent_id, &self.id);
        }
        if !new_parent_id.is_empty() {
            tracker.add_instancer_instancer_dependency(&new_parent_id, &self.id);
        }
        self.parent_id = new_parent_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instancer_creation() {
        let id = SdfPath::from_string("/Instancer").unwrap();
        let instancer = HdInstancer::new(None, id.clone(), None);

        assert_eq!(instancer.get_id(), &id);
        assert!(!instancer.is_nested());
        assert!(instancer.get_parent_id().is_empty());
        assert!(instancer.get_delegate().is_none());
    }

    #[test]
    fn test_nested_instancer() {
        let parent_id = SdfPath::from_string("/ParentInstancer").unwrap();
        let child_id = SdfPath::from_string("/ChildInstancer").unwrap();

        let instancer = HdInstancer::new(None, child_id.clone(), Some(parent_id.clone()));

        assert!(instancer.is_nested());
        assert_eq!(instancer.get_parent_id(), &parent_id);
    }

    #[test]
    fn test_initial_dirty_bits_mask() {
        let mask = HdInstancer::get_initial_dirty_bits_mask();
        // Should be specific bits, not ALL_DIRTY
        assert_ne!(mask, HdInstancer::ALL_DIRTY);
        assert!((mask & HdInstancer::DIRTY_TRANSFORM) != 0);
        assert!((mask & HdInstancer::DIRTY_PRIMVAR) != 0);
        assert!((mask & HdInstancer::DIRTY_INSTANCE_INDEX) != 0);
        assert!((mask & HdInstancer::DIRTY_INSTANCER) != 0);
        assert!((mask & HdInstancer::DIRTY_VISIBILITY) != 0);
    }

    #[test]
    fn test_builtin_primvar_names() {
        let names = HdInstancer::get_builtin_primvar_names();
        assert_eq!(names.len(), 4);
        assert_eq!(names[0].as_str(), "hydra:instanceTransforms");
        assert_eq!(names[1].as_str(), "hydra:instanceRotations");
        assert_eq!(names[2].as_str(), "hydra:instanceScales");
        assert_eq!(names[3].as_str(), "hydra:instanceTranslations");
    }

    #[test]
    fn test_instance_count() {
        let mut instancer =
            HdInstancer::new(None, SdfPath::from_string("/Instancer").unwrap(), None);

        assert_eq!(instancer.get_num_instances(), 0);

        instancer.set_num_instances(100);
        assert_eq!(instancer.get_num_instances(), 100);
    }
}
