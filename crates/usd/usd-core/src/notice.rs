//! USD Notice system for change notifications.
//!
//! Port of pxr/usd/usd/notice.h/cpp
//!
//! Provides notice types for tracking changes to USD stages and their contents.

use super::object::Object;
use super::stage::Stage;
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// StageNotice (Base)
// ============================================================================

/// Base class for UsdStage notices.
///
/// Matches C++ `UsdNotice::StageNotice`.
#[derive(Clone)]
pub struct StageNotice {
    /// Weak reference to the stage.
    stage: Weak<Stage>,
}

impl StageNotice {
    /// Creates a new stage notice.
    ///
    /// Matches C++ `StageNotice(const UsdStageWeakPtr &stage)`.
    pub fn new(stage: Weak<Stage>) -> Self {
        Self { stage }
    }

    /// Returns the stage associated with this notice.
    ///
    /// Matches C++ `GetStage()`.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.stage.upgrade()
    }

    /// Returns the weak reference to the stage.
    pub fn stage_weak(&self) -> &Weak<Stage> {
        &self.stage
    }
}

// ============================================================================
// StageContentsChanged
// ============================================================================

/// Ultra-conservative notice sent when the given UsdStage's contents have changed.
///
/// Matches C++ `UsdNotice::StageContentsChanged`.
///
/// This notice is sent when any authoring is performed in any of the stage's
/// participatory layers, in the thread performing the authoring, after the
/// affected UsdStage has reconfigured itself in response to the authored changes.
pub struct StageContentsChanged {
    base: StageNotice,
}

impl StageContentsChanged {
    /// Creates a new StageContentsChanged notice.
    ///
    /// Matches C++ `StageContentsChanged(const UsdStageWeakPtr& stage)`.
    pub fn new(stage: Weak<Stage>) -> Self {
        Self {
            base: StageNotice::new(stage),
        }
    }

    /// Returns the stage associated with this notice.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.base.stage()
    }
}

// ============================================================================
// PrimResyncType
// ============================================================================

/// Classification of prim resync operations.
///
/// Matches C++ `UsdNotice::ObjectsChanged::PrimResyncType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimResyncType {
    /// Prim was renamed (source path).
    RenameSource,
    /// Prim was renamed (destination path).
    RenameDestination,
    /// Prim was reparented (source path).
    ReparentSource,
    /// Prim was reparented (destination path).
    ReparentDestination,
    /// Prim was renamed and reparented (source path).
    RenameAndReparentSource,
    /// Prim was renamed and reparented (destination path).
    RenameAndReparentDestination,
    /// Prim was deleted.
    Delete,
    /// Prim resync but prim stack unchanged.
    UnchangedPrimStack,
    /// Other type of resync.
    Other,
    /// Invalid (not resynced).
    Invalid,
}

impl PrimResyncType {
    /// Returns true if this is a rename operation.
    pub fn is_rename(&self) -> bool {
        matches!(
            self,
            Self::RenameSource
                | Self::RenameDestination
                | Self::RenameAndReparentSource
                | Self::RenameAndReparentDestination
        )
    }

    /// Returns true if this is a reparent operation.
    pub fn is_reparent(&self) -> bool {
        matches!(
            self,
            Self::ReparentSource
                | Self::ReparentDestination
                | Self::RenameAndReparentSource
                | Self::RenameAndReparentDestination
        )
    }

    /// Returns true if this is a source path.
    pub fn is_source(&self) -> bool {
        matches!(
            self,
            Self::RenameSource | Self::ReparentSource | Self::RenameAndReparentSource
        )
    }

    /// Returns true if this is a destination path.
    pub fn is_destination(&self) -> bool {
        matches!(
            self,
            Self::RenameDestination
                | Self::ReparentDestination
                | Self::RenameAndReparentDestination
        )
    }
}

// ============================================================================
// ObjectsChanged
// ============================================================================

/// Notice sent in response to authored changes that affect UsdObjects.
///
/// Matches C++ `UsdNotice::ObjectsChanged`.
///
/// Provides detailed information about:
/// - Object resyncs (structural changes)
/// - Resolved asset path resyncs
/// - Changed info (non-structural changes)
#[derive(Clone)]
pub struct ObjectsChanged {
    base: StageNotice,
    /// Map from paths to change entries (resync changes).
    resync_changes: HashMap<Path, Vec<ChangeEntry>>,
    /// Map from paths to change entries (info changes only).
    info_changes: HashMap<Path, Vec<ChangeEntry>>,
    /// Map from paths to change entries (asset path changes).
    asset_path_changes: HashMap<Path, Vec<ChangeEntry>>,
    /// Namespace edits information.
    namespace_edits: NamespaceEditsInfo,
}

impl usd_tf::notice::Notice for ObjectsChanged {
    fn notice_type_name() -> &'static str {
        "UsdNotice::ObjectsChanged"
    }
}

/// Information about a change entry.
#[derive(Debug, Clone)]
pub struct ChangeEntry {
    /// Changed fields (metadata keys, etc.).
    pub changed_fields: Vec<Token>,
}

impl ChangeEntry {
    /// Creates a new empty change entry.
    pub fn new() -> Self {
        Self {
            changed_fields: Vec::new(),
        }
    }

    /// Creates a new change entry with the specified changed fields.
    pub fn with_changed_fields(changed_fields: Vec<Token>) -> Self {
        Self { changed_fields }
    }
}

impl Default for ChangeEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about namespace edits.
#[derive(Debug, Clone, Default)]
pub struct NamespaceEditsInfo {
    /// Map from prim paths to resync info.
    pub prim_resyncs: HashMap<Path, PrimResyncInfo>,
    /// List of renamed properties (old path, new name).
    pub renamed_properties: Vec<(Path, Token)>,
}

/// Information about a prim resync.
#[derive(Debug, Clone)]
pub struct PrimResyncInfo {
    /// Type of resync.
    pub resync_type: PrimResyncType,
    /// Associated path (for rename/reparent operations).
    pub associated_path: Option<Path>,
}

impl ObjectsChanged {
    /// Creates a new ObjectsChanged notice.
    ///
    /// Matches C++ constructor with all parameters.
    pub fn new(
        stage: Weak<Stage>,
        resync_changes: HashMap<Path, Vec<ChangeEntry>>,
        info_changes: HashMap<Path, Vec<ChangeEntry>>,
        asset_path_changes: HashMap<Path, Vec<ChangeEntry>>,
        namespace_edits: NamespaceEditsInfo,
    ) -> Self {
        Self {
            base: StageNotice::new(stage),
            resync_changes,
            info_changes,
            asset_path_changes,
            namespace_edits,
        }
    }

    /// Creates a new ObjectsChanged notice with only resync changes.
    ///
    /// Matches C++ constructor with only resync changes.
    pub fn new_with_resyncs(
        stage: Weak<Stage>,
        resync_changes: HashMap<Path, Vec<ChangeEntry>>,
    ) -> Self {
        Self::new(
            stage,
            resync_changes,
            HashMap::new(),
            HashMap::new(),
            NamespaceEditsInfo::default(),
        )
    }

    /// Returns the stage associated with this notice.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.base.stage()
    }

    /// Return true if obj was possibly affected by the layer changes.
    ///
    /// Matches C++ `AffectedObject(const UsdObject &obj)`.
    pub fn affected_object(&self, obj: &Object) -> bool {
        self.resynced_object(obj)
            || self.resolved_asset_paths_resynced(obj)
            || self.changed_info_only(obj)
    }

    /// Return true if obj was resynced by the layer changes.
    ///
    /// Matches C++ `ResyncedObject(const UsdObject &obj)`.
    pub fn resynced_object(&self, obj: &Object) -> bool {
        self.resynced_path(obj.path())
    }

    /// Return true if obj path was resynced.
    ///
    /// Matches C++ `ResyncedObject` for a path.
    pub fn resynced_path(&self, path: &Path) -> bool {
        // Check if path or any ancestor is in resync_changes
        self.find_longest_prefix(&self.resync_changes, path)
            .is_some()
    }

    /// Return true if asset path values in obj were resynced.
    ///
    /// Matches C++ `ResolvedAssetPathsResynced(const UsdObject &obj)`.
    pub fn resolved_asset_paths_resynced(&self, obj: &Object) -> bool {
        self.resolved_asset_paths_resynced_path(obj.path())
    }

    /// Return true if path has resolved asset paths resynced.
    pub fn resolved_asset_paths_resynced_path(&self, path: &Path) -> bool {
        self.find_longest_prefix(&self.asset_path_changes, path)
            .is_some()
    }

    /// Return true if obj was changed but not resynced.
    ///
    /// Matches C++ `ChangedInfoOnly(const UsdObject &obj)`.
    pub fn changed_info_only(&self, obj: &Object) -> bool {
        self.changed_info_only_path(obj.path())
    }

    /// Return true if path has changed info only.
    pub fn changed_info_only_path(&self, path: &Path) -> bool {
        self.info_changes.contains_key(path)
    }

    /// Returns an iterator over resynced paths.
    ///
    /// Matches C++ `GetResyncedPaths()`.
    pub fn get_resynced_paths(&self) -> PathRange {
        PathRange::new(&self.resync_changes)
    }

    /// Returns an iterator over changed info only paths.
    ///
    /// Matches C++ `GetChangedInfoOnlyPaths()`.
    pub fn get_changed_info_only_paths(&self) -> PathRange {
        PathRange::new(&self.info_changes)
    }

    /// Returns an iterator over resolved asset paths resynced paths.
    ///
    /// Matches C++ `GetResolvedAssetPathsResyncedPaths()`.
    pub fn get_resolved_asset_paths_resynced_paths(&self) -> PathRange {
        PathRange::new(&self.asset_path_changes)
    }

    /// Returns the set of changed fields for an object.
    ///
    /// Matches C++ `GetChangedFields(const UsdObject &obj)`.
    pub fn get_changed_fields(&self, obj: &Object) -> Vec<Token> {
        self.get_changed_fields_path(obj.path())
    }

    /// Returns the set of changed fields for a path.
    ///
    /// Matches C++ `GetChangedFields(const SdfPath &path)`.
    pub fn get_changed_fields_path(&self, path: &Path) -> Vec<Token> {
        // Check resynced paths first
        if let Some(entries) = self.resync_changes.get(path) {
            return self.collect_changed_fields(entries);
        }

        // Then check info changes
        if let Some(entries) = self.info_changes.get(path) {
            return self.collect_changed_fields(entries);
        }

        Vec::new()
    }

    /// Returns true if there are any changed fields for an object.
    ///
    /// Matches C++ `HasChangedFields(const UsdObject &obj)`.
    pub fn has_changed_fields(&self, obj: &Object) -> bool {
        self.has_changed_fields_path(obj.path())
    }

    /// Returns true if there are any changed fields for a path.
    ///
    /// Matches C++ `HasChangedFields(const SdfPath &path)`.
    pub fn has_changed_fields_path(&self, path: &Path) -> bool {
        !self.get_changed_fields_path(path).is_empty()
    }

    /// Returns the type of resync for a prim path.
    ///
    /// Matches C++ `GetPrimResyncType(const SdfPath &primPath, SdfPath *associatedPrimPath)`.
    pub fn get_prim_resync_type(
        &self,
        prim_path: &Path,
        associated_prim_path: &mut Option<Path>,
    ) -> PrimResyncType {
        if let Some(resync_info) = self.namespace_edits.prim_resyncs.get(prim_path) {
            *associated_prim_path = resync_info.associated_path.clone();
            return resync_info.resync_type;
        }
        PrimResyncType::Invalid
    }

    /// Returns the list of renamed properties.
    ///
    /// Matches C++ `GetRenamedProperties()`.
    pub fn get_renamed_properties(&self) -> &[(Path, Token)] {
        &self.namespace_edits.renamed_properties
    }

    // Helper methods

    fn find_longest_prefix(
        &self,
        map: &HashMap<Path, Vec<ChangeEntry>>,
        path: &Path,
    ) -> Option<Path> {
        // Find the longest prefix of path that exists in map
        let mut current = path.clone();
        loop {
            if map.contains_key(&current) {
                return Some(current);
            }
            let parent = current.get_parent_path();
            if parent == current || parent.is_empty() {
                // Reached root or empty path
                break;
            }
            current = parent;
        }
        None
    }

    fn collect_changed_fields(&self, entries: &[ChangeEntry]) -> Vec<Token> {
        let mut fields = Vec::new();
        for entry in entries {
            fields.extend(entry.changed_fields.iter().cloned());
        }
        // Remove duplicates and sort
        fields.sort();
        fields.dedup();
        fields
    }
}

// ============================================================================
// PathRange
// ============================================================================

/// An iterable range of paths to objects that have changed.
///
/// Matches C++ `UsdNotice::ObjectsChanged::PathRange`.
pub struct PathRange {
    changes: Option<HashMap<Path, Vec<ChangeEntry>>>,
}

impl PathRange {
    /// Creates a new PathRange.
    fn new(changes: &HashMap<Path, Vec<ChangeEntry>>) -> Self {
        Self {
            changes: Some(changes.clone()),
        }
    }

    /// Returns true if this range contains any paths.
    pub fn is_empty(&self) -> bool {
        self.changes.as_ref().is_none_or(|c| c.is_empty())
    }

    /// Returns the number of paths in this range.
    pub fn len(&self) -> usize {
        self.changes.as_ref().map_or(0, |c| c.len())
    }

    /// Returns an iterator over the paths.
    pub fn iter(&self) -> PathRangeIterator {
        PathRangeIterator::new(self.changes.as_ref())
    }

    /// Returns an iterator to the specified path if it exists.
    pub fn find(&self, path: &Path) -> Option<Path> {
        self.changes.as_ref().and_then(|c| {
            if c.contains_key(path) {
                Some(path.clone())
            } else {
                None
            }
        })
    }

    /// Converts to a vector of paths.
    pub fn to_vec(&self) -> Vec<Path> {
        self.iter().collect()
    }
}

impl IntoIterator for &PathRange {
    type Item = Path;
    type IntoIter = PathRangeIterator;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over paths in a PathRange.
pub struct PathRangeIterator {
    paths: Vec<Path>,
    index: usize,
}

impl PathRangeIterator {
    fn new(changes: Option<&HashMap<Path, Vec<ChangeEntry>>>) -> Self {
        let paths = changes
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default();
        Self { paths, index: 0 }
    }
}

impl Iterator for PathRangeIterator {
    type Item = Path;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.paths.len() {
            let path = self.paths[self.index].clone();
            self.index += 1;
            Some(path)
        } else {
            None
        }
    }
}

// ============================================================================
// StageEditTargetChanged
// ============================================================================

/// Notice sent when a stage's EditTarget has changed.
///
/// Matches C++ `UsdNotice::StageEditTargetChanged`.
pub struct StageEditTargetChanged {
    base: StageNotice,
}

impl StageEditTargetChanged {
    /// Creates a new StageEditTargetChanged notice.
    ///
    /// Matches C++ `StageEditTargetChanged(const UsdStageWeakPtr &stage)`.
    pub fn new(stage: Weak<Stage>) -> Self {
        Self {
            base: StageNotice::new(stage),
        }
    }

    /// Returns the stage associated with this notice.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.base.stage()
    }
}

// ============================================================================
// LayerMutingChanged
// ============================================================================

/// Notice sent after a set of layers have been newly muted or unmuted.
///
/// Matches C++ `UsdNotice::LayerMutingChanged`.
pub struct LayerMutingChanged {
    base: StageNotice,
    muted_layers: Vec<String>,
    unmuted_layers: Vec<String>,
}

impl LayerMutingChanged {
    /// Creates a new LayerMutingChanged notice.
    ///
    /// Matches C++ `LayerMutingChanged(const UsdStageWeakPtr &stage, ...)`.
    pub fn new(stage: Weak<Stage>, muted_layers: Vec<String>, unmuted_layers: Vec<String>) -> Self {
        Self {
            base: StageNotice::new(stage),
            muted_layers,
            unmuted_layers,
        }
    }

    /// Returns the stage associated with this notice.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.base.stage()
    }

    /// Returns the list of newly muted layers.
    ///
    /// Matches C++ `GetMutedLayers()`.
    pub fn get_muted_layers(&self) -> &[String] {
        &self.muted_layers
    }

    /// Returns the list of newly unmuted layers.
    ///
    /// Matches C++ `GetUnmutedLayers()`.
    pub fn get_unmuted_layers(&self) -> &[String] {
        &self.unmuted_layers
    }
}

// ============================================================================
// UsdNotice (Container)
// ============================================================================

/// Container class for Usd notices.
///
/// Matches C++ `UsdNotice`.
pub struct UsdNotice;

impl UsdNotice {
    // Notice types are exported as separate types, not nested classes
}
