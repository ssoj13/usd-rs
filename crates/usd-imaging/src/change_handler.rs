//! USD Notice handler for change tracking.
//!
//! Port of C++ UsdImagingDelegate::_OnUsdObjectsChanged logic.
//!
//! Listens for `UsdNotice::ObjectsChanged` from the stage and translates
//! changed USD paths into Hydra dirty bits, queuing them for the delegate's
//! `apply_pending_updates()`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use usd_core::Stage;
use usd_core::notice::ObjectsChanged;
use usd_hd::HdDirtyBits;
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_sdf::Path;
use usd_tf::Token;
use usd_tf::notice::ListenerKey;

// ============================================================================
// PendingChanges: accumulated changes from notice callbacks
// ============================================================================

/// Accumulated changes from one or more ObjectsChanged notices.
///
/// Matches C++ `_usdPathsToResync` + `_usdPathsToUpdate`.
#[derive(Debug, Default)]
pub struct PendingChanges {
    /// Paths requiring full resync (structural changes).
    pub paths_to_resync: Vec<Path>,
    /// Paths requiring info-only update, with changed fields per path.
    pub paths_to_update: HashMap<Path, Vec<Token>>,
}

impl PendingChanges {
    /// Returns true if there are no pending changes.
    pub fn is_empty(&self) -> bool {
        self.paths_to_resync.is_empty() && self.paths_to_update.is_empty()
    }

    /// Takes all pending changes, leaving self empty.
    pub fn take(&mut self) -> PendingChanges {
        PendingChanges {
            paths_to_resync: std::mem::take(&mut self.paths_to_resync),
            paths_to_update: std::mem::take(&mut self.paths_to_update),
        }
    }

    /// Clear all pending changes.
    pub fn clear(&mut self) {
        self.paths_to_resync.clear();
        self.paths_to_update.clear();
    }
}

// ============================================================================
// ChangeHandler: notice listener
// ============================================================================

/// Handles ObjectsChanged notices from the USD stage.
///
/// Accumulates changed paths into `PendingChanges` which can be drained
/// by the delegate during `apply_pending_updates()`.
///
/// # Usage
///
/// ```ignore
/// let handler = ChangeHandler::new(Arc::downgrade(&stage));
/// handler.register();
/// // ... stage edits happen, handler accumulates changes ...
/// let changes = handler.drain();
/// // process changes
/// handler.revoke();
/// ```
pub struct ChangeHandler {
    /// Weak ref to the stage we're listening to.
    stage: Weak<Stage>,
    /// Accumulated pending changes (thread-safe).
    pending: Arc<Mutex<PendingChanges>>,
    /// Listener key for revoking the notice registration.
    listener_key: Mutex<Option<ListenerKey>>,
}

impl ChangeHandler {
    /// Create a new change handler for the given stage.
    pub fn new(stage: Weak<Stage>) -> Arc<Self> {
        Arc::new(Self {
            stage,
            pending: Arc::new(Mutex::new(PendingChanges::default())),
            listener_key: Mutex::new(None),
        })
    }

    /// Register for ObjectsChanged notices on the global TfNotice registry.
    ///
    /// Matches C++ `TfNotice::Register(self, &This::_OnUsdObjectsChanged, _stage)`.
    pub fn register(self: &Arc<Self>) {
        let handler = Arc::clone(self);
        let stage_weak = self.stage.clone();

        let key = usd_tf::notice::register_global::<ObjectsChanged, _>(move |notice| {
            // Verify the notice is for our stage
            if let Some(notice_stage) = notice.stage() {
                if let Some(our_stage) = stage_weak.upgrade() {
                    if !Arc::ptr_eq(&notice_stage, &our_stage) {
                        return; // Not our stage
                    }
                } else {
                    return; // Our stage is gone
                }
            }
            handler.on_objects_changed(notice);
        });

        *self.listener_key.lock().expect("Lock poisoned") = Some(key);
    }

    /// Revoke the notice listener registration.
    ///
    /// Matches C++ `TfNotice::Revoke(_objectsChangedNoticeKey)`.
    pub fn revoke(&self) {
        if let Some(key) = self.listener_key.lock().expect("Lock poisoned").take() {
            usd_tf::notice::revoke(key);
        }
    }

    /// Drain all pending changes, returning them and clearing the internal queue.
    pub fn drain(&self) -> PendingChanges {
        self.pending.lock().expect("Lock poisoned").take()
    }

    /// Check if there are pending changes.
    pub fn has_pending(&self) -> bool {
        !self.pending.lock().expect("Lock poisoned").is_empty()
    }

    /// Core notice handler — translates ObjectsChanged into PendingChanges.
    ///
    /// Port of C++ `UsdImagingDelegate::_OnUsdObjectsChanged()`.
    fn on_objects_changed(&self, notice: &ObjectsChanged) {
        let mut pending = self.pending.lock().expect("Lock poisoned");

        // Phase 1: Collect paths to resync (structural changes).
        // C++: pathsToResync = notice.GetResyncedPaths()
        let resynced = notice.get_resynced_paths();
        for path in &resynced {
            if path.is_property_path() {
                // Property path — resync the owning prim instead.
                // C++: _usdPathsToResync.emplace_back(path.GetPrimPath())
                pending.paths_to_resync.push(path.get_prim_path());
            } else {
                pending.paths_to_resync.push(path.clone());
            }
        }

        // Phase 2: Resolved asset path resyncs also go to resync.
        // C++: assetPathsToResync = notice.GetResolvedAssetPathsResyncedPaths()
        let asset_resyncs = notice.get_resolved_asset_paths_resynced_paths();
        for path in &asset_resyncs {
            pending.paths_to_resync.push(path.clone());
        }

        // Phase 3: Changed info only — sparse property invalidation.
        // C++: pathsToUpdate = notice.GetChangedInfoOnlyPaths()
        let info_paths = notice.get_changed_info_only_paths();
        for path in &info_paths {
            if path.is_absolute_root_or_prim_path() {
                // Prim path — only record if there are changed fields.
                let fields = notice.get_changed_fields_path(&path);
                if !fields.is_empty() {
                    let entry = pending.paths_to_update.entry(path.clone()).or_default();
                    entry.extend(fields);
                }
            } else if path.is_property_path() {
                // Property path — always record.
                pending.paths_to_update.entry(path.clone()).or_default();
            }
        }

        log::debug!(
            "[ChangeHandler] Notice processed: {} resyncs, {} updates",
            pending.paths_to_resync.len(),
            pending.paths_to_update.len()
        );
    }
}

impl Drop for ChangeHandler {
    fn drop(&mut self) {
        self.revoke();
    }
}

// ============================================================================
// Dirty bits mapping: USD field tokens -> HdDirtyBits
// ============================================================================

/// Map a set of changed USD field tokens to Hydra dirty bits.
///
/// This is a simplified mapping; full C++ uses per-adapter
/// `ProcessPropertyChange()` for fine-grained control.
pub fn fields_to_dirty_bits(fields: &[Token]) -> HdDirtyBits {
    let mut bits: HdDirtyBits = 0;

    for field in fields {
        let name = field.as_str();
        bits |= match name {
            // Geometry
            "points" | "extent" => HdRprimDirtyBits::DIRTY_POINTS | HdRprimDirtyBits::DIRTY_EXTENT,
            "normals" | "primvars:normals" => HdRprimDirtyBits::DIRTY_NORMALS,
            "widths" => HdRprimDirtyBits::DIRTY_WIDTHS,
            "faceVertexIndices" | "faceVertexCounts" | "holeIndices" | "curveVertexCounts"
            | "pointCount" => HdRprimDirtyBits::DIRTY_TOPOLOGY,

            // Transform
            "xformOpOrder" => HdRprimDirtyBits::DIRTY_TRANSFORM,

            // Visibility
            "visibility" | "active" => HdRprimDirtyBits::DIRTY_VISIBILITY,

            // Material
            "material:binding" => HdRprimDirtyBits::DIRTY_MATERIAL_ID,

            // Display
            "doubleSided" => HdRprimDirtyBits::DIRTY_DOUBLE_SIDED,
            "displayColor" | "displayOpacity" => HdRprimDirtyBits::DIRTY_PRIMVAR,

            // Subdivision
            "subdivisionScheme" | "interpolateBoundary" | "faceVaryingLinearInterpolation" => {
                HdRprimDirtyBits::DIRTY_SUBDIV_TAGS | HdRprimDirtyBits::DIRTY_TOPOLOGY
            }

            // Catch-all for xformOp:* tokens
            _ if name.starts_with("xformOp:") => HdRprimDirtyBits::DIRTY_TRANSFORM,
            // Catch-all for primvars:*
            _ if name.starts_with("primvars:") => HdRprimDirtyBits::DIRTY_PRIMVAR,
            // Unknown field — mark primvar dirty as safe fallback
            _ => HdRprimDirtyBits::DIRTY_PRIMVAR,
        };
    }

    bits
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_changes_empty() {
        let pc = PendingChanges::default();
        assert!(pc.is_empty());
    }

    #[test]
    fn test_pending_changes_take() {
        let mut pc = PendingChanges::default();
        pc.paths_to_resync.push(Path::from("/World/Mesh"));
        assert!(!pc.is_empty());

        let taken = pc.take();
        assert!(pc.is_empty());
        assert_eq!(taken.paths_to_resync.len(), 1);
    }

    #[test]
    fn test_fields_to_dirty_points() {
        let fields = vec![Token::new("points")];
        let bits = fields_to_dirty_bits(&fields);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_POINTS, 0);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_EXTENT, 0);
    }

    #[test]
    fn test_fields_to_dirty_transform() {
        let fields = vec![Token::new("xformOpOrder"), Token::new("xformOp:translate")];
        let bits = fields_to_dirty_bits(&fields);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_TRANSFORM, 0);
    }

    #[test]
    fn test_fields_to_dirty_visibility() {
        let fields = vec![Token::new("visibility")];
        let bits = fields_to_dirty_bits(&fields);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_VISIBILITY, 0);
    }

    #[test]
    fn test_fields_to_dirty_material() {
        let fields = vec![Token::new("material:binding")];
        let bits = fields_to_dirty_bits(&fields);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_MATERIAL_ID, 0);
    }

    #[test]
    fn test_fields_to_dirty_topology() {
        let fields = vec![Token::new("faceVertexIndices")];
        let bits = fields_to_dirty_bits(&fields);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_TOPOLOGY, 0);
    }

    #[test]
    fn test_fields_to_dirty_primvar_prefix() {
        let fields = vec![Token::new("primvars:displayColor")];
        let bits = fields_to_dirty_bits(&fields);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_PRIMVAR, 0);
    }

    #[test]
    fn test_fields_to_dirty_subdiv() {
        let fields = vec![Token::new("subdivisionScheme")];
        let bits = fields_to_dirty_bits(&fields);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_SUBDIV_TAGS, 0);
        assert_ne!(bits & HdRprimDirtyBits::DIRTY_TOPOLOGY, 0);
    }

    #[test]
    fn test_change_handler_create() {
        let stage = Stage::create_in_memory(usd_core::InitialLoadSet::LoadAll).unwrap();
        let handler = ChangeHandler::new(Arc::downgrade(&stage));
        assert!(!handler.has_pending());
    }

    #[test]
    fn test_change_handler_register_revoke() {
        let stage = Stage::create_in_memory(usd_core::InitialLoadSet::LoadAll).unwrap();
        let handler = ChangeHandler::new(Arc::downgrade(&stage));
        handler.register();
        assert!(!handler.has_pending());
        handler.revoke();
    }
}
