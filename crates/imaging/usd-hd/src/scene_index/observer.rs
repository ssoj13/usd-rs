
//! Scene index observer for tracking scene changes.

use std::sync::{Arc, Weak};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// Forward declare for data source types
// These will be defined in the data_source module

/// Set of data source locators identifying dirty regions within prims.
/// See `HdDataSourceLocator` in OpenUSD for hierarchical locator semantics.
pub type HdDataSourceLocatorSet = crate::data_source::HdDataSourceLocatorSet;
// Use but don't re-export to avoid ambiguity with base module
use crate::data_source::HdContainerDataSourceHandle;

/// Entry for a prim added to the scene.
///
/// Contains the path, type, and optionally the initial state.
/// Note that a prim might already exist at the path, in which case
/// this acts as a resync or type change notification.
#[derive(Debug, Clone)]
pub struct AddedPrimEntry {
    /// Path to the prim
    pub prim_path: SdfPath,
    /// Type of the prim (e.g., "Mesh", "Camera")
    pub prim_type: TfToken,
    /// Optional initial data source for the prim
    pub data_source: Option<HdContainerDataSourceHandle>,
}

impl AddedPrimEntry {
    /// Create a new added prim entry.
    pub fn new(prim_path: SdfPath, prim_type: TfToken) -> Self {
        Self {
            prim_path,
            prim_type,
            data_source: None,
        }
    }

    /// Create a new added prim entry with data source.
    pub fn with_data_source(
        prim_path: SdfPath,
        prim_type: TfToken,
        data_source: HdContainerDataSourceHandle,
    ) -> Self {
        Self {
            prim_path,
            prim_type,
            data_source: Some(data_source),
        }
    }
}

/// Entry for a prim removed from the scene.
///
/// Removal is hierarchical: if /Path is removed, /Path/child is also
/// considered removed.
#[derive(Debug, Clone)]
pub struct RemovedPrimEntry {
    /// Path to the removed prim (and its subtree)
    pub prim_path: SdfPath,
}

impl RemovedPrimEntry {
    /// Create a new removed prim entry.
    pub fn new(prim_path: SdfPath) -> Self {
        Self { prim_path }
    }
}

/// Entry for a prim that has been dirtied.
///
/// The dirty locators identify which data sources need to be re-pulled.
/// Locators are hierarchical: if "primvars" is dirtied, "primvars/color"
/// is also considered dirty. This only affects the named prim; descendants
/// are unaffected.
#[derive(Debug, Clone)]
pub struct DirtiedPrimEntry {
    /// Path to the dirtied prim
    pub prim_path: SdfPath,
    /// Set of locators that are dirty
    pub dirty_locators: HdDataSourceLocatorSet,
}

impl DirtiedPrimEntry {
    /// Create a new dirtied prim entry.
    pub fn new(prim_path: SdfPath, dirty_locators: HdDataSourceLocatorSet) -> Self {
        Self {
            prim_path,
            dirty_locators,
        }
    }
}

/// Entry for a prim that has been renamed or reparented.
///
/// This affects the prim and all its descendants.
#[derive(Debug, Clone)]
pub struct RenamedPrimEntry {
    /// Original path
    pub old_prim_path: SdfPath,
    /// New path
    pub new_prim_path: SdfPath,
}

impl RenamedPrimEntry {
    /// Create a new renamed prim entry.
    pub fn new(old_prim_path: SdfPath, new_prim_path: SdfPath) -> Self {
        Self {
            old_prim_path,
            new_prim_path,
        }
    }
}

/// Type aliases for entry collections.

/// Collection of added prim entries for batch notifications.
pub type AddedPrimEntries = Vec<AddedPrimEntry>;

/// Collection of removed prim entries for batch notifications.
pub type RemovedPrimEntries = Vec<RemovedPrimEntry>;

/// Collection of dirtied prim entries for batch notifications.
pub type DirtiedPrimEntries = Vec<DirtiedPrimEntry>;

/// Collection of renamed prim entries for batch notifications.
pub type RenamedPrimEntries = Vec<RenamedPrimEntry>;

/// Observer interface for scene index changes.
///
/// Observers receive notifications when prims are added, removed, dirtied,
/// or renamed in a scene index.
///
/// # Thread Safety
///
/// Observer notifications are NOT thread-safe and should be called from
/// a single thread. Query methods on the scene index ARE thread-safe.
pub trait HdSceneIndexObserver: Send + Sync {
    /// Notification that prims have been added to the scene.
    ///
    /// The set of scene prims should match the set from traversing
    /// via GetChildPrimPaths. Each prim has a path and type. It's possible
    /// for PrimsAdded to be called for prims that already exist; in that
    /// case, observers should update the prim type and resync.
    fn prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]);

    /// Notification that prims have been removed from the scene.
    ///
    /// This message is hierarchical: if /Path is removed, /Path/child
    /// is considered removed as well.
    fn prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]);

    /// Notification that prim data sources have been invalidated.
    ///
    /// This message is NOT hierarchical on primPath; if /Path is dirtied,
    /// /Path/child is not necessarily dirtied. However, data source locators
    /// ARE hierarchical: if "primvars" is dirtied, "primvars/color" is
    /// considered dirtied as well.
    fn prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]);

    /// Notification that prims (and their descendants) have been renamed
    /// or reparented.
    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]);
}

/// Weak reference to an observer.
pub type HdSceneIndexObserverWeakHandle = Weak<dyn HdSceneIndexObserver>;

/// Strong reference to an observer.
/// No RwLock — matches C++ shared_ptr semantics. Observer trait is &self + Send + Sync,
/// so implementations use interior mutability. This avoids recursive read-lock
/// deadlocks when notification cascades re-enter the scene index chain.
pub type HdSceneIndexObserverHandle = Arc<dyn HdSceneIndexObserver>;

/// Utility to convert renamed entries into equivalent removed and added notices.
///
/// Port of HdSceneIndexObserver::ConvertPrimsRenamedToRemovedAndAdded.
/// For each rename, emits a removal at old path, then BFS-traverses from new path
/// and emits additions for the prim and all descendants.
pub fn convert_prims_renamed_to_removed_and_added(
    sender: &dyn HdSceneIndexBase,
    renamed_entries: &[RenamedPrimEntry],
) -> (RemovedPrimEntries, AddedPrimEntries) {
    let mut removed_entries = Vec::with_capacity(renamed_entries.len());
    let mut added_entries = Vec::with_capacity(renamed_entries.len());

    for entry in renamed_entries {
        if entry.old_prim_path != entry.new_prim_path {
            removed_entries.push(RemovedPrimEntry::new(entry.old_prim_path.clone()));

            let mut work_queue = vec![entry.new_prim_path.clone()];
            while let Some(path) = work_queue.pop() {
                let prim = sender.get_prim(&path);
                added_entries.push(AddedPrimEntry::new(path.clone(), prim.prim_type));

                let child_paths = sender.get_child_prim_paths(&path);
                for child in child_paths.into_iter().rev() {
                    work_queue.push(child);
                }
            }
        }
    }

    (removed_entries, added_entries)
}

// Forward declaration for HdSceneIndexBase trait
// This avoids circular dependency with base.rs
use super::base::HdSceneIndexBase;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_added_prim_entry() {
        let entry = AddedPrimEntry::new(
            SdfPath::from_string("/World").unwrap(),
            TfToken::new("Mesh"),
        );
        assert_eq!(entry.prim_path.as_str(), "/World");
        assert_eq!(entry.prim_type.as_str(), "Mesh");
    }

    #[test]
    fn test_removed_prim_entry() {
        let entry = RemovedPrimEntry::new(SdfPath::from_string("/World").unwrap());
        assert_eq!(entry.prim_path.as_str(), "/World");
    }

    #[test]
    fn test_dirtied_prim_entry() {
        let entry = DirtiedPrimEntry::new(
            SdfPath::from_string("/World").unwrap(),
            HdDataSourceLocatorSet::new(),
        );
        assert_eq!(entry.prim_path.as_str(), "/World");
    }

    #[test]
    fn test_renamed_prim_entry() {
        let entry = RenamedPrimEntry::new(
            SdfPath::from_string("/Old").unwrap(),
            SdfPath::from_string("/New").unwrap(),
        );
        assert_eq!(entry.old_prim_path.as_str(), "/Old");
        assert_eq!(entry.new_prim_path.as_str(), "/New");
    }
}
