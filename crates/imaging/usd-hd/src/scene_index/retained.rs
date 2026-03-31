
//! Retained scene index - mutable scene container.

use super::base::{HdSceneIndexBase, HdSceneIndexBaseImpl, SdfPathVector, TfTokenVector};
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
};
use super::prim::{HdContainerDataSourceHandle as PrimDataSourceHandle, HdSceneIndexPrim};
use crate::data_source::HdRetainedContainerDataSource;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Entry for adding a prim to retained scene index.
#[derive(Clone)]
pub struct RetainedAddedPrimEntry {
    /// Path to the prim
    pub prim_path: SdfPath,
    /// Type of the prim
    pub prim_type: TfToken,
    /// Data source for the prim
    pub data_source: Option<PrimDataSourceHandle>,
}

impl RetainedAddedPrimEntry {
    /// Create a new retained added prim entry.
    pub fn new(
        prim_path: SdfPath,
        prim_type: TfToken,
        data_source: Option<PrimDataSourceHandle>,
    ) -> Self {
        Self {
            prim_path,
            prim_type,
            data_source,
        }
    }
}

/// Concrete scene index that can be externally populated and dirtied.
///
/// This is a simple mutable container for scene data. Prims can be added,
/// removed, and dirtied via the public API.
///
/// # C++ SdfPathTable parity
///
/// C++ HdRetainedSceneIndex uses SdfPathTable as its backing store. SdfPathTable
/// **implicitly inserts all ancestor paths** when any path is inserted. A query
/// for an implicit ancestor returns a prim with an empty (non-null) data source,
/// making it truthy / "defined". GetChildPrimPaths on an implicit ancestor also
/// returns its direct children.
///
/// This Rust implementation replicates that behaviour by maintaining `ancestors`
/// (paths that exist implicitly because a descendant was explicitly added) and
/// by storing the full ancestor chain inside `children`.
///
/// # Thread Safety
///
/// Queries (GetPrim, GetChildPrimPaths) are thread-safe via RwLock.
/// Mutations (AddPrims, RemovePrims) require mutable access.
pub struct HdRetainedSceneIndex {
    /// Base implementation for observers
    base: HdSceneIndexBaseImpl,
    /// Prim storage - maps path to prim (explicitly added paths only)
    prims: HashMap<SdfPath, HdSceneIndexPrim>,
    /// Child relationships - maps parent path to direct children.
    /// Populated for EVERY ancestor of every explicitly added prim, mirroring
    /// the implicit ancestor insertion of C++ SdfPathTable.
    children: HashMap<SdfPath, Vec<SdfPath>>,
    /// Paths that exist implicitly (as ancestors of explicitly added prims)
    /// but were never explicitly added. Used to synthesise defined-but-empty
    /// prims in get_prim(), matching C++ SdfPathTable behaviour.
    ancestors: std::collections::HashSet<SdfPath>,
}

impl HdRetainedSceneIndex {
    /// Create a new retained scene index.
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSceneIndexBaseImpl::new(),
            prims: HashMap::new(),
            children: HashMap::new(),
            ancestors: std::collections::HashSet::new(),
        }))
    }

    /// Add prims to the scene.
    ///
    /// Each entry specifies a path, type, and data source. The scene index
    /// takes ownership and uses them to answer queries. This generates a
    /// PrimsAdded notification.
    pub fn add_prims(&mut self, entries: &[RetainedAddedPrimEntry]) {
        if entries.is_empty() {
            return;
        }

        let mut added_entries = Vec::with_capacity(entries.len());

        for entry in entries {
            // Create the prim
            let prim = HdSceneIndexPrim::new(entry.prim_type.clone(), entry.data_source.clone());

            // Store the prim
            self.prims.insert(entry.prim_path.clone(), prim);

            // Update child relationships
            self.add_child_relationship(&entry.prim_path);

            // Prepare notification entry
            added_entries.push(AddedPrimEntry::new(
                entry.prim_path.clone(),
                entry.prim_type.clone(),
            ));
        }

        // Send notification. Safe: observers may call get_prim on sender (reading
        // prims/children) while we mutate base (notify_depth); no overlapping access.
        if !added_entries.is_empty() {
            let self_ptr = self as *const Self;
            // SAFETY: self_ptr is valid for 'static (callback lifetime)
            #[allow(unsafe_code)]
            self.base
                .send_prims_added(unsafe { &*self_ptr }, &added_entries);
        }
    }

    /// Remove prims from the scene.
    ///
    /// Removes the prim subtree starting at each entry's path. This generates
    /// a PrimsRemoved notification.
    pub fn remove_prims(&mut self, entries: &[RemovedPrimEntry]) {
        if entries.is_empty() {
            return;
        }

        for entry in entries {
            self.remove_prim_subtree(&entry.prim_path);
        }

        // Send notification.
        if !entries.is_empty() {
            let self_ptr = self as *const Self;
            // SAFETY: self_ptr is valid for callback lifetime
            #[allow(unsafe_code)]
            self.base.send_prims_removed(unsafe { &*self_ptr }, entries);
        }
    }

    /// Invalidate prim data.
    ///
    /// C++ parity: filters entries to only include paths present in the
    /// internal table. This is useful because emulation shares a render
    /// index and some actions dirty all prims, including those not in
    /// this scene index.
    pub fn dirty_prims(&mut self, entries: &[DirtiedPrimEntry]) {
        if entries.is_empty() {
            return;
        }

        // Filter to only paths that exist in our entries map
        let filtered: Vec<DirtiedPrimEntry> = entries
            .iter()
            .filter(|e| self.prims.contains_key(&e.prim_path))
            .cloned()
            .collect();

        if filtered.is_empty() {
            return;
        }

        let self_ptr = self as *const Self;
        // SAFETY: self_ptr is valid for callback lifetime
        #[allow(unsafe_code)]
        self.base
            .send_prims_dirtied(unsafe { &*self_ptr }, &filtered);
    }

    /// Remove a prim and all its descendants.
    fn remove_prim_subtree(&mut self, path: &SdfPath) {
        // Get all children first (to avoid borrow issues)
        let mut to_remove = vec![path.clone()];
        let mut i = 0;

        while i < to_remove.len() {
            let current = &to_remove[i];

            // Find children
            if let Some(children) = self.children.get(current) {
                to_remove.extend(children.iter().cloned());
            }

            i += 1;
        }

        // Remove all prims and their children entries.
        for path_to_remove in &to_remove {
            self.prims.remove(path_to_remove);
            self.children.remove(path_to_remove);
            self.ancestors.remove(path_to_remove);
        }

        // Remove from parent's child list; also clean up stale ancestor entries
        // for ancestor paths that no longer have any children.
        if let Some(parent) = self.get_parent_path(path) {
            if let Some(siblings) = self.children.get_mut(&parent) {
                siblings.retain(|p| p != path);
                if siblings.is_empty() {
                    self.children.remove(&parent);
                    // Parent is now empty — remove from ancestors too if it has
                    // no explicit prim entry.
                    if !self.prims.contains_key(&parent) {
                        self.ancestors.remove(&parent);
                    }
                }
            }
        }
    }

    /// Add child relationships for a path and all its implicit ancestors.
    ///
    /// C++ SdfPathTable parity: inserting a path also implicitly inserts all
    /// ancestors, which become discoverable via GetChildPrimPaths even if they
    /// were never explicitly added as prims. We replicate this by walking the
    /// entire ancestor chain and recording parent→child links at every level.
    fn add_child_relationship(&mut self, path: &SdfPath) {
        if path.is_absolute_root_path() {
            return;
        }

        let mut current = path.clone();
        loop {
            let parent = current.get_parent_path();
            let children_vec = self.children.entry(parent.clone()).or_default();
            if !children_vec.contains(&current) {
                children_vec.push(current.clone());
            }
            if parent.is_absolute_root_path() {
                break;
            }
            // Track parent as implicit ancestor if it has no explicit prim entry.
            // Do not track the absolute root; it's always conceptually present.
            if !self.prims.contains_key(&parent) {
                self.ancestors.insert(parent.clone());
            }
            current = parent;
        }
    }

    /// Get parent path helper.
    fn get_parent_path(&self, path: &SdfPath) -> Option<SdfPath> {
        if path.is_absolute_root_path() {
            None
        } else {
            Some(path.get_parent_path())
        }
    }
}

impl HdSceneIndexBase for HdRetainedSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        match self.prims.get(prim_path) {
            None => {
                // C++ SdfPathTable parity: implicit ancestors (inserted when a
                // descendant was explicitly added) return a defined prim with an
                // empty (non-null) data source, making them truthy / "defined".
                if self.ancestors.contains(prim_path) || self.children.contains_key(prim_path) {
                    HdSceneIndexPrim::new(
                        TfToken::empty(),
                        Some(HdRetainedContainerDataSource::new_empty()),
                    )
                } else {
                    HdSceneIndexPrim::empty()
                }
            }
            Some(prim) => {
                if prim.data_source.is_none() {
                    // C++ parity: null dataSource -> return empty container,
                    // not null. Matches HdRetainedContainerDataSource::New().
                    HdSceneIndexPrim::new(
                        prim.prim_type.clone(),
                        Some(HdRetainedContainerDataSource::new_empty()),
                    )
                } else {
                    prim.clone()
                }
            }
        }
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        self.children
            .get(prim_path)
            .cloned()
            .unwrap_or_else(Vec::new)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.remove_observer(observer);
    }

    fn set_display_name(&mut self, name: String) {
        self.base.set_display_name(name);
    }

    fn add_tag(&mut self, tag: TfToken) {
        self.base.add_tag(tag);
    }

    fn remove_tag(&mut self, tag: &TfToken) {
        self.base.remove_tag(tag);
    }

    fn has_tag(&self, tag: &TfToken) -> bool {
        self.base.has_tag(tag)
    }

    fn get_tags(&self) -> TfTokenVector {
        self.base.get_tags()
    }

    fn get_display_name(&self) -> String {
        if self.base.get_display_name().is_empty() {
            "HdRetainedSceneIndex".to_string()
        } else {
            self.base.get_display_name().to_string()
        }
    }
}

impl Default for HdRetainedSceneIndex {
    fn default() -> Self {
        Self {
            base: HdSceneIndexBaseImpl::new(),
            prims: HashMap::new(),
            children: HashMap::new(),
            ancestors: std::collections::HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_index::observer::RemovedPrimEntry;

    #[test]
    fn test_retained_scene_creation() {
        let scene = HdRetainedSceneIndex::new();
        let scene_lock = scene.read();

        // Should start empty
        assert!(
            scene_lock
                .get_prim(&SdfPath::absolute_root())
                .data_source
                .is_none()
        );
    }

    #[test]
    fn test_add_prims() {
        let scene = HdRetainedSceneIndex::new();
        let mut scene_lock = scene.write();

        let entries = vec![RetainedAddedPrimEntry::new(
            SdfPath::from_string("/World").unwrap(),
            TfToken::new("Xform"),
            None,
        )];

        scene_lock.add_prims(&entries);

        let prim = scene_lock.get_prim(&SdfPath::from_string("/World").unwrap());
        assert_eq!(prim.prim_type.as_str(), "Xform");
    }

    #[test]
    fn test_get_child_prim_paths() {
        let scene = HdRetainedSceneIndex::new();
        let mut scene_lock = scene.write();

        // Add parent
        scene_lock.add_prims(&[RetainedAddedPrimEntry::new(
            SdfPath::from_string("/World").unwrap(),
            TfToken::new("Xform"),
            None,
        )]);

        // Add children
        scene_lock.add_prims(&[
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/World/Cube").unwrap(),
                TfToken::new("Mesh"),
                None,
            ),
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/World/Sphere").unwrap(),
                TfToken::new("Mesh"),
                None,
            ),
        ]);

        let children = scene_lock.get_child_prim_paths(&SdfPath::from_string("/World").unwrap());
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn test_remove_prims() {
        let scene = HdRetainedSceneIndex::new();
        let mut scene_lock = scene.write();

        // Add a prim with data source so is_defined() returns true
        // (C++ is_defined checks data_source != null only)
        use crate::data_source::HdRetainedContainerDataSource;
        let ds: Arc<dyn crate::data_source::HdContainerDataSource> =
            HdRetainedContainerDataSource::new_empty();
        scene_lock.add_prims(&[RetainedAddedPrimEntry::new(
            SdfPath::from_string("/World").unwrap(),
            TfToken::new("Xform"),
            Some(ds),
        )]);

        assert!(
            scene_lock
                .get_prim(&SdfPath::from_string("/World").unwrap())
                .is_defined()
        );

        // Remove it
        let entries = vec![RemovedPrimEntry::new(
            SdfPath::from_string("/World").unwrap(),
        )];
        scene_lock.remove_prims(&entries);

        assert!(
            !scene_lock
                .get_prim(&SdfPath::from_string("/World").unwrap())
                .is_defined()
        );
    }
}
