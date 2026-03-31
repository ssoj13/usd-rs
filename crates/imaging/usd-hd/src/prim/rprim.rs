
//! Rprim - Renderable primitive base trait.
//!
//! Rprims represent renderable geometry in Hydra. This includes:
//! - Meshes (HdMesh)
//! - Basis curves (HdBasisCurves)
//! - Points (HdPoints)
//!
//! # Responsibilities
//!
//! - Manage prim identity (SdfPath)
//! - Track dirty bits for change propagation
//! - Sync with scene data via HdSceneDelegate
//! - Provide visibility, material, and transform access
//! - Support instancing via instancer_id
//! - Manage reprs and draw items
//!
//! # Change Tracking
//!
//! Rprims use dirty bits to track what has changed:
//! - `DIRTY_VISIBILITY` - Visibility changed
//! - `DIRTY_MATERIAL_ID` - Material binding changed
//! - `DIRTY_TRANSFORM` - Transform changed
//! - `DIRTY_PRIMVAR` - Primvar data changed
//! - `DIRTY_TOPOLOGY` - Topology changed
//! - `DIRTY_INSTANCES` - Instance data changed

use super::{HdRenderParam, HdSceneDelegate};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Base trait for renderable primitives.
///
/// All geometry prims (mesh, curves, points) implement this trait.
/// Matches C++ HdRprim interface from pxr/imaging/hd/rprim.h.
pub trait HdRprim {
    // =========================================================================
    // Dirty bit constants - re-exported from HdChangeTracker::RprimDirtyBits.
    // These MUST match the bit positions in change_tracker.rs / C++ changeTracker.h.
    // =========================================================================

    /// Clean state - no changes.
    const CLEAN: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::CLEAN;

    /// Prim ID changed (bit 2).
    const DIRTY_PRIM_ID: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_PRIM_ID;

    /// Extent (bounding box) changed (bit 3).
    const DIRTY_EXTENT: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_EXTENT;

    /// Display style changed (bit 4).
    const DIRTY_DISPLAY_STYLE: HdDirtyBits =
        crate::change_tracker::HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;

    /// Points/vertices changed (bit 5).
    const DIRTY_POINTS: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_POINTS;

    /// Primvar data changed (bit 6).
    const DIRTY_PRIMVAR: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_PRIMVAR;

    /// Material binding changed (bit 7).
    const DIRTY_MATERIAL_ID: HdDirtyBits =
        crate::change_tracker::HdRprimDirtyBits::DIRTY_MATERIAL_ID;

    /// Topology changed (bit 8).
    const DIRTY_TOPOLOGY: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_TOPOLOGY;

    /// Transform changed (bit 9).
    const DIRTY_TRANSFORM: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_TRANSFORM;

    /// Visibility changed (bit 10).
    const DIRTY_VISIBILITY: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_VISIBILITY;

    /// Normals changed (bit 11).
    const DIRTY_NORMALS: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_NORMALS;

    /// Double-sided flag changed (bit 12).
    const DIRTY_DOUBLE_SIDED: HdDirtyBits =
        crate::change_tracker::HdRprimDirtyBits::DIRTY_DOUBLE_SIDED;

    /// Cull style changed (bit 13).
    const DIRTY_CULL_STYLE: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_CULL_STYLE;

    /// Subdivision tags changed (bit 14).
    const DIRTY_SUBDIV_TAGS: HdDirtyBits =
        crate::change_tracker::HdRprimDirtyBits::DIRTY_SUBDIV_TAGS;

    /// Widths (curves) changed (bit 15).
    const DIRTY_WIDTHS: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_WIDTHS;

    /// Instancer changed (bit 16).
    const DIRTY_INSTANCER: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_INSTANCER;

    /// Instance indices changed (bit 17).
    const DIRTY_INSTANCE_INDEX: HdDirtyBits =
        crate::change_tracker::HdRprimDirtyBits::DIRTY_INSTANCE_INDEX;

    /// Representation changed (bit 18).
    const DIRTY_REPR: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_REPR;

    /// Render tag changed (bit 19).
    const DIRTY_RENDER_TAG: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_RENDER_TAG;

    /// Computation primvar descriptor changed (bit 20).
    const DIRTY_COMPUTATION_PRIMVAR_DESC: HdDirtyBits =
        crate::change_tracker::HdRprimDirtyBits::DIRTY_COMPUTATION_PRIMVAR_DESC;

    /// Categories changed (bit 21).
    const DIRTY_CATEGORIES: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::DIRTY_CATEGORIES;

    /// Volume field binding changed (bit 22).
    const DIRTY_VOLUME_FIELD: HdDirtyBits =
        crate::change_tracker::HdRprimDirtyBits::DIRTY_VOLUME_FIELD;

    /// All dirty bits except Varying (matches C++ ~Varying).
    const ALL_DIRTY: HdDirtyBits = crate::change_tracker::HdRprimDirtyBits::ALL_DIRTY;

    // =========================================================================
    // Required methods
    // =========================================================================

    /// Get prim identifier.
    fn get_id(&self) -> &SdfPath;

    /// Get current dirty bits.
    fn get_dirty_bits(&self) -> HdDirtyBits;

    /// Set dirty bits.
    fn set_dirty_bits(&mut self, bits: HdDirtyBits);

    /// Get instancer id if instanced.
    fn get_instancer_id(&self) -> Option<&SdfPath>;

    /// Pull invalidated scene data and prepare/update renderable representation.
    ///
    /// Called in parallel from worker threads; must be thread-safe.
    /// Matches C++ `HdRprim::Sync(HdSceneDelegate*, HdRenderParam*, HdDirtyBits*, TfToken const&)`.
    ///
    /// # Arguments
    ///
    /// * `delegate` - Scene delegate to query data from
    /// * `render_param` - Render delegate parameters (thread-safe)
    /// * `dirty_bits` - Which aspects need syncing (in/out)
    /// * `repr_token` - Which representation needs updating
    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
        repr_token: &Token,
    );

    /// Propagate dirty bits: add additional bits based on those already set.
    /// Called before delegate sync. Matches C++ `_PropagateDirtyBits`.
    /// Default: pass through unchanged.
    fn propagate_dirty_bits(&self, bits: HdDirtyBits) -> HdDirtyBits {
        bits
    }

    /// Initialize representation for the given repr token.
    /// Called prior to dirty bit propagation & sync, the first time the repr
    /// is used. `dirty_bits` is in/out (may set additional bits).
    /// Matches C++ `_InitRepr`.
    /// Default: no-op.
    fn init_repr(&mut self, _repr_token: &Token, _dirty_bits: &mut HdDirtyBits) {}

    /// Update the authored repr selector before repr initialization.
    ///
    /// OpenUSD performs this during pre-sync when `InitRepr` or `DirtyRepr`
    /// bits are present so the prim can resolve collection-driven repr state
    /// before `InitRepr(...)` potentially allocates new draw items.
    ///
    /// Default: no-op for prims that do not store repr-selector state.
    fn update_repr_selector(&mut self, _repr_selector: super::HdReprSelector) {}

    // =========================================================================
    // Provided methods with default implementations
    // =========================================================================

    /// Get initial dirty bits mask for new prims.
    fn get_initial_dirty_bits_mask() -> HdDirtyBits
    where
        Self: Sized,
    {
        Self::ALL_DIRTY
    }

    /// Check if prim is visible.
    fn is_visible(&self) -> bool {
        true
    }

    /// Get material id.
    fn get_material_id(&self) -> Option<&SdfPath> {
        None
    }

    /// Mark bits as clean.
    fn mark_clean(&mut self, bits: HdDirtyBits) {
        let current = self.get_dirty_bits();
        self.set_dirty_bits(current & !bits);
    }

    /// Mark bits as dirty.
    fn mark_dirty(&mut self, bits: HdDirtyBits) {
        let current = self.get_dirty_bits();
        self.set_dirty_bits(current | bits);
    }

    /// Check if any bits are dirty.
    fn is_dirty(&self) -> bool {
        self.get_dirty_bits() != Self::CLEAN
    }

    /// Check if specific bits are dirty.
    fn is_dirty_bits(&self, bits: HdDirtyBits) -> bool {
        (self.get_dirty_bits() & bits) != 0
    }

    // =========================================================================
    // State management (C++ prim_id, CanSkipDirtyBitPropagation, etc.)
    // =========================================================================

    /// Get unique prim id for id renders. Matches C++ `GetPrimId`.
    /// Default: -1 (unassigned).
    fn get_prim_id(&self) -> i32 {
        -1
    }

    /// Set unique prim id. Matches C++ `SetPrimId`.
    /// Default: no-op (override to store).
    fn set_prim_id(&mut self, _prim_id: i32) {}

    /// Early exit from dirty bit propagation and sync.
    /// Returns true if the prim can skip all work (e.g. invisible prims).
    /// Matches C++ `CanSkipDirtyBitPropagationAndSync`.
    fn can_skip_dirty_bit_propagation_and_sync(&self, bits: HdDirtyBits) -> bool {
        // C++ rprim.cpp:57-67: skip if invisible AND neither DirtyVisibility
        // nor NewRepr bits are set (those require sync even when invisible).
        use crate::change_tracker::HdRprimDirtyBits;
        let mask = Self::DIRTY_VISIBILITY | HdRprimDirtyBits::NEW_REPR;
        if !self.is_visible() && (bits & mask) == 0 {
            return true;
        }
        false
    }

    /// Propagate rprim dirty bits (public entry point wrapping propagate_dirty_bits).
    ///
    /// Applies base cascading logic before calling subclass `_PropagateDirtyBits`:
    /// - DirtyComputationPrimvarDesc -> forces DirtyPoints|DirtyNormals|DirtyWidths|DirtyPrimvar
    /// - DirtyDisplayStyle -> forces DirtyTopology
    /// - DirtyTopology -> forces DirtyPoints|DirtyNormals|DirtyPrimvar
    ///
    /// Matches C++ `HdRprim::PropagateRprimDirtyBits` (rprim.cpp:71-97).
    fn propagate_rprim_dirty_bits(&self, mut bits: HdDirtyBits) -> HdDirtyBits {
        use crate::change_tracker::HdRprimDirtyBits;

        // If computation primvar descriptors changed, assume all primvars dirty.
        if (bits & HdRprimDirtyBits::DIRTY_COMPUTATION_PRIMVAR_DESC) != 0 {
            bits |= HdRprimDirtyBits::DIRTY_POINTS
                | HdRprimDirtyBits::DIRTY_NORMALS
                | HdRprimDirtyBits::DIRTY_WIDTHS
                | HdRprimDirtyBits::DIRTY_PRIMVAR;
        }

        // When refine level changes, topology becomes dirty.
        if (bits & HdRprimDirtyBits::DIRTY_DISPLAY_STYLE) != 0 {
            bits |= HdRprimDirtyBits::DIRTY_TOPOLOGY;
        }

        // If topology changes, all dependent bits become dirty.
        if (bits & HdRprimDirtyBits::DIRTY_TOPOLOGY) != 0 {
            bits |= HdRprimDirtyBits::DIRTY_POINTS
                | HdRprimDirtyBits::DIRTY_NORMALS
                | HdRprimDirtyBits::DIRTY_PRIMVAR;
        }

        // Let subclasses propagate bits.
        self.propagate_dirty_bits(bits)
    }

    // =========================================================================
    // Lifecycle
    // =========================================================================

    /// Finalize before destruction. Matches C++ `Finalize(HdRenderParam*)`.
    fn finalize(&mut self, _render_param: Option<&dyn HdRenderParam>) {}

    /// Required built-in primvar names for this prim type.
    fn get_builtin_primvar_names() -> Vec<Token>
    where
        Self: Sized,
    {
        Vec::new()
    }

    /// Update render tag. Called when render tag changes.
    fn update_render_tag(&mut self, _render_tag: &Token) {}

    // =========================================================================
    // Delegate wrapper convenience methods (P2-2)
    // =========================================================================

    /// Get extent from scene delegate. Convenience for `delegate.get_extent(id)`.
    fn get_extent(&self, delegate: &dyn HdSceneDelegate) -> usd_gf::Range3d {
        delegate.get_extent(self.get_id())
    }

    /// Get primvar descriptors from scene delegate.
    fn get_primvar_descriptors(
        &self,
        delegate: &dyn HdSceneDelegate,
        interpolation: crate::enums::HdInterpolation,
    ) -> crate::scene_delegate::HdPrimvarDescriptorVector {
        delegate.get_primvar_descriptors(self.get_id(), interpolation)
    }

    /// Get primvar value from scene delegate.
    fn get_primvar(&self, delegate: &dyn HdSceneDelegate, name: &Token) -> usd_vt::Value {
        delegate.get(self.get_id(), name)
    }

    /// Get indexed primvar from scene delegate.
    fn get_indexed_primvar(
        &self,
        delegate: &dyn HdSceneDelegate,
        name: &Token,
    ) -> (usd_vt::Value, Option<Vec<i32>>) {
        delegate.get_indexed_primvar(self.get_id(), name)
    }

    /// Get points primvar from scene delegate.
    fn get_points(&self, delegate: &dyn HdSceneDelegate) -> usd_vt::Value {
        use crate::tokens::POINTS;
        delegate.get(self.get_id(), &POINTS)
    }

    /// Get normals primvar from scene delegate.
    fn get_normals(&self, delegate: &dyn HdSceneDelegate) -> usd_vt::Value {
        use crate::tokens::NORMALS;
        delegate.get(self.get_id(), &NORMALS)
    }

    /// Get render tag from scene delegate.
    fn get_render_tag(&self, delegate: &dyn HdSceneDelegate) -> Token {
        delegate.get_render_tag(self.get_id())
    }

    // =========================================================================
    // Visibility / instancer updates (P2-3)
    // =========================================================================

    /// Update visibility from scene delegate if dirty.
    /// Matches C++ `_UpdateVisibility`. Default: no-op (override to store).
    fn update_visibility(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _dirty_bits: &mut HdDirtyBits,
    ) {
    }

    /// Update instancer reference from scene delegate if dirty.
    /// Matches C++ `_UpdateInstancer`. Default: no-op.
    fn update_instancer(&mut self, _delegate: &dyn HdSceneDelegate, _dirty_bits: &mut HdDirtyBits) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple mock rprim for testing
    struct MockRprim {
        id: SdfPath,
        dirty_bits: HdDirtyBits,
        instancer_id: Option<SdfPath>,
        prim_id: i32,
        visible: bool,
    }

    impl HdRprim for MockRprim {
        fn get_id(&self) -> &SdfPath {
            &self.id
        }

        fn get_dirty_bits(&self) -> HdDirtyBits {
            self.dirty_bits
        }

        fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
            self.dirty_bits = bits;
        }

        fn get_instancer_id(&self) -> Option<&SdfPath> {
            self.instancer_id.as_ref()
        }

        fn sync(
            &mut self,
            _delegate: &dyn HdSceneDelegate,
            _render_param: Option<&dyn HdRenderParam>,
            dirty_bits: &mut HdDirtyBits,
            _repr_token: &Token,
        ) {
            *dirty_bits = Self::CLEAN;
            self.dirty_bits = Self::CLEAN;
        }

        fn get_prim_id(&self) -> i32 {
            self.prim_id
        }

        fn set_prim_id(&mut self, prim_id: i32) {
            self.prim_id = prim_id;
        }

        fn is_visible(&self) -> bool {
            self.visible
        }
    }

    #[test]
    fn test_dirty_bits() {
        let mut prim = MockRprim {
            id: SdfPath::from_string("/Test").unwrap(),
            dirty_bits: MockRprim::CLEAN,
            instancer_id: None,
            prim_id: -1,
            visible: true,
        };

        assert!(!prim.is_dirty());
        assert_eq!(prim.get_dirty_bits(), MockRprim::CLEAN);

        prim.mark_dirty(MockRprim::DIRTY_VISIBILITY);
        assert!(prim.is_dirty());
        assert!(prim.is_dirty_bits(MockRprim::DIRTY_VISIBILITY));

        prim.mark_clean(MockRprim::DIRTY_VISIBILITY);
        assert!(!prim.is_dirty());
    }

    #[test]
    fn test_dirty_bits_combinations() {
        let mut prim = MockRprim {
            id: SdfPath::from_string("/Test").unwrap(),
            dirty_bits: MockRprim::CLEAN,
            instancer_id: None,
            prim_id: -1,
            visible: true,
        };

        prim.mark_dirty(MockRprim::DIRTY_VISIBILITY | MockRprim::DIRTY_TRANSFORM);
        assert!(prim.is_dirty_bits(MockRprim::DIRTY_VISIBILITY));
        assert!(prim.is_dirty_bits(MockRprim::DIRTY_TRANSFORM));
        assert!(!prim.is_dirty_bits(MockRprim::DIRTY_MATERIAL_ID));

        prim.mark_clean(MockRprim::DIRTY_VISIBILITY);
        assert!(!prim.is_dirty_bits(MockRprim::DIRTY_VISIBILITY));
        assert!(prim.is_dirty_bits(MockRprim::DIRTY_TRANSFORM));
    }

    #[test]
    fn test_initial_dirty_bits() {
        let mask = MockRprim::get_initial_dirty_bits_mask();
        assert_eq!(mask, MockRprim::ALL_DIRTY);
    }

    #[test]
    fn test_prim_id() {
        let mut prim = MockRprim {
            id: SdfPath::from_string("/Test").unwrap(),
            dirty_bits: MockRprim::CLEAN,
            instancer_id: None,
            prim_id: -1,
            visible: true,
        };

        assert_eq!(prim.get_prim_id(), -1);
        prim.set_prim_id(42);
        assert_eq!(prim.get_prim_id(), 42);
    }

    #[test]
    fn test_can_skip_invisible() {
        let prim = MockRprim {
            id: SdfPath::from_string("/Test").unwrap(),
            dirty_bits: MockRprim::CLEAN,
            instancer_id: None,
            prim_id: -1,
            visible: false,
        };

        // Invisible with DirtyVisibility set -> cannot skip (need to sync vis change)
        assert!(!prim.can_skip_dirty_bit_propagation_and_sync(MockRprim::DIRTY_VISIBILITY));
        // Invisible with clean (no vis/newrepr bits) -> can skip
        assert!(prim.can_skip_dirty_bit_propagation_and_sync(MockRprim::CLEAN));
        // Invisible with other dirty bits (not vis/newrepr) -> can skip
        assert!(prim.can_skip_dirty_bit_propagation_and_sync(MockRprim::DIRTY_TOPOLOGY));
    }
}
