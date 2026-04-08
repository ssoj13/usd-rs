//! SelectionSceneIndex - Selection scene index for UsdImaging.
//!
//! Port of pxr/usdImaging/usdImaging/selectionSceneIndex.cpp
//!
//! A filtering scene index that overlays HdSelectionsSchema on selected prims.
//! Mirrors the C++ `_PrimSource` pattern: GetPrim returns an overlay container
//! that merges the input prim's data with a selections vector data source for
//! any prim in the selection set.

use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::Arc;
use usd_hd::data_source::{HdContainerDataSourceHandle, HdRetainedTypedSampledDataSource};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, wire_filter_to_input,
};
use usd_hd::schema::{HdSelectionSchemaBuilder, HdSelectionsSchema};
use usd_hd::{HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocatorSet};
use usd_sdf::Path;
use usd_tf::Token;

#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SELECTIONS: LazyLock<Token> = LazyLock::new(|| Token::new("selections"));
}

// ---------------------------------------------------------------------------
// Selection info shared between scene index and data sources
// ---------------------------------------------------------------------------

/// Per-prim selection entries. Each entry counts how many times
/// this prim was selected (one selection -> one HdSelectionSchema element).
#[derive(Clone, Debug, Default)]
struct SelectionEntry {
    count: usize,
}

/// Shared selection state keyed by scene index path.
/// Uses BTreeMap so `delete_prefix` can use ordered iteration.
#[derive(Debug, Default)]
struct SelectionInfo {
    prim_to_selections: BTreeMap<Path, SelectionEntry>,
}

impl SelectionInfo {
    /// Remove all entries whose path has `prefix` as a prefix (or equals it).
    /// Matches C++ `_DeletePrefix`.
    fn delete_prefix(&mut self, prefix: &Path) {
        self.prim_to_selections.retain(|p, _| !p.has_prefix(prefix));
    }
}

/// Build selections vector data source for a prim.
///
/// Creates `count` selection entries, each with fullySelected = true,
/// matching the C++ `_ToDs(_Selections)` chain.
fn build_selections_ds(entry: &SelectionEntry) -> HdDataSourceBaseHandle {
    let mut selections: Vec<HdContainerDataSourceHandle> = Vec::with_capacity(entry.count);
    for _ in 0..entry.count {
        let sel = HdSelectionSchemaBuilder::new()
            .set_fully_selected(HdRetainedTypedSampledDataSource::new(true))
            .build();
        selections.push(sel);
    }
    HdSelectionsSchema::build_retained(&selections) as HdDataSourceBaseHandle
}

// ---------------------------------------------------------------------------
// PrimSourceDataSource - overlay container injecting selections
// ---------------------------------------------------------------------------

/// Container data source that overlays selections onto input prim data.
///
/// Mirrors C++ `_PrimSource`. For get_names/get, if the prim is in the
/// selection set the `selections` key returns a vector of HdSelectionSchema
/// entries. All other keys delegate to the input data source.
#[derive(Clone)]
struct PrimSourceDataSource {
    input: HdContainerDataSourceHandle,
    selection_info: Arc<RwLock<SelectionInfo>>,
    prim_path: Path,
}

impl std::fmt::Debug for PrimSourceDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimSourceDataSource")
            .field("path", &self.prim_path)
            .finish()
    }
}

impl usd_hd::HdDataSourceBase for PrimSourceDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimSourceDataSource {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.input.get_names();
        let info = self.selection_info.read();
        if info.prim_to_selections.contains_key(&self.prim_path) {
            names.push(tokens::SELECTIONS.clone());
        }
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::SELECTIONS {
            let info = self.selection_info.read();
            if let Some(entry) = info.prim_to_selections.get(&self.prim_path) {
                return Some(build_selections_ds(entry));
            }
            return None;
        }
        self.input.get(name)
    }
}

// ---------------------------------------------------------------------------
// SelectionSceneIndex
// ---------------------------------------------------------------------------

/// A filtering scene index that overlays HdSelectionsSchema on selected prims.
///
/// When `add_selection` is called with a USD path, the scene index records
/// the selection and emits PrimsDirtied. `GetPrim` wraps every prim's data
/// source in a `PrimSourceDataSource` that injects the `selections` key
/// for prims in the selection set, matching the C++ `_PrimSource` pattern.
pub struct SelectionSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Shared selection state accessible by PrimSourceDataSource instances.
    selection_info: Arc<RwLock<SelectionInfo>>,
}

impl std::fmt::Debug for SelectionSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectionSceneIndex").finish()
    }
}

impl SelectionSceneIndex {
    /// Creates a new selection scene index wrapping `input`.
    pub fn new(input: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input.clone())),
            selection_info: Arc::new(RwLock::new(SelectionInfo::default())),
        }));
        log::info!("[selection] about to wire");
        wire_filter_to_input(&result, &input);
        log::info!("[selection] wire done");
        result
    }

    /// Adds a selection for the given USD path.
    ///
    /// Records the prim as selected and sends PrimsDirtied for the
    /// selections locator so downstream observers update highlights.
    pub fn add_selection(&mut self, usd_path: &Path) {
        let is_new = {
            let mut info = self.selection_info.write();
            let entry = info.prim_to_selections.entry(usd_path.clone()).or_default();
            entry.count += 1;
            entry.count == 1
        };

        if is_new {
            let locators =
                HdDataSourceLocatorSet::from_locator(HdSelectionsSchema::get_default_locator());
            let entries = vec![DirtiedPrimEntry {
                prim_path: usd_path.clone(),
                dirty_locators: locators,
            }];
            // Match `_ref`: downstream consumers may inspect the sender's view.
            self.base.base().send_prims_dirtied(self, &entries);
        }
    }

    /// Clears all selections. Sends PrimsDirtied for every previously selected path.
    pub fn clear_selection(&mut self) {
        let paths: Vec<Path> = {
            let info = self.selection_info.read();
            if info.prim_to_selections.is_empty() {
                return;
            }
            info.prim_to_selections.keys().cloned().collect()
        };

        self.selection_info.write().prim_to_selections.clear();

        let locators =
            HdDataSourceLocatorSet::from_locator(HdSelectionsSchema::get_default_locator());
        let entries: Vec<DirtiedPrimEntry> = paths
            .into_iter()
            .map(|p| DirtiedPrimEntry {
                prim_path: p,
                dirty_locators: locators.clone(),
            })
            .collect();
        // Match `_ref`: downstream consumers may inspect the sender's view.
        self.base.base().send_prims_dirtied(self, &entries);
    }

    /// Checks if a path is currently selected.
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selection_info
            .read()
            .prim_to_selections
            .contains_key(path)
    }

    /// Returns all currently selected paths.
    pub fn get_selected_paths(&self) -> Vec<Path> {
        self.selection_info
            .read()
            .prim_to_selections
            .keys()
            .cloned()
            .collect()
    }
}

impl HdSceneIndexBase for SelectionSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            let mut prim = input_locked.get_prim(prim_path);
            if let Some(ds) = prim.data_source.take() {
                // Wrap in PrimSourceDataSource that overlays selections
                let overlay = PrimSourceDataSource {
                    input: ds,
                    selection_info: Arc::clone(&self.selection_info),
                    prim_path: prim_path.clone(),
                };
                prim.data_source = Some(Arc::new(overlay));
            }
            return prim;
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            return input_locked.get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "SelectionSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for SelectionSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        // Clean up selection state for removed prims (matches C++ _DeletePrefix)
        {
            let mut info = self.selection_info.write();
            for entry in entries {
                info.delete_prefix(&entry.prim_path);
            }
        }
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

/// Handle type for SelectionSceneIndex.
pub type SelectionSceneIndexHandle = Arc<RwLock<SelectionSceneIndex>>;

/// Creates a new selection scene index.
pub fn create_selection_scene_index(input: HdSceneIndexHandle) -> SelectionSceneIndexHandle {
    SelectionSceneIndex::new(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::data_source::HdRetainedContainerDataSource;

    #[test]
    fn test_prim_source_get_names_includes_selections() {
        let info = Arc::new(RwLock::new(SelectionInfo::default()));
        let path = Path::from_string("/World/Mesh").unwrap();
        let input = HdRetainedContainerDataSource::new_empty();

        // Not selected - should NOT contain "selections"
        let ds = PrimSourceDataSource {
            input: input.clone() as HdContainerDataSourceHandle,
            selection_info: Arc::clone(&info),
            prim_path: path.clone(),
        };
        assert!(!ds.get_names().contains(&tokens::SELECTIONS));

        // Select it
        info.write()
            .prim_to_selections
            .insert(path.clone(), SelectionEntry { count: 1 });

        let names = ds.get_names();
        assert!(names.contains(&tokens::SELECTIONS));
    }

    #[test]
    fn test_prim_source_get_returns_selections_ds() {
        let info = Arc::new(RwLock::new(SelectionInfo::default()));
        let path = Path::from_string("/A").unwrap();
        let input = HdRetainedContainerDataSource::new_empty();

        info.write()
            .prim_to_selections
            .insert(path.clone(), SelectionEntry { count: 2 });

        let ds = PrimSourceDataSource {
            input: input as HdContainerDataSourceHandle,
            selection_info: Arc::clone(&info),
            prim_path: path,
        };

        // "selections" should return a vector data source
        assert!(ds.get(&tokens::SELECTIONS).is_some());
    }

    #[test]
    fn test_prim_source_delegates_other_keys() {
        let info = Arc::new(RwLock::new(SelectionInfo::default()));
        let path = Path::from_string("/A").unwrap();

        let foo_token = Token::new("foo");
        let foo_ds: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(42i32);
        let input = HdRetainedContainerDataSource::from_entries(&[(foo_token.clone(), foo_ds)]);

        let ds = PrimSourceDataSource {
            input: input as HdContainerDataSourceHandle,
            selection_info: Arc::clone(&info),
            prim_path: path,
        };

        // "foo" delegates to input
        assert!(ds.get(&foo_token).is_some());
        // "selections" is None (not selected)
        assert!(ds.get(&tokens::SELECTIONS).is_none());
    }

    #[test]
    fn test_selection_tracking() {
        let mut scene = SelectionSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            selection_info: Arc::new(RwLock::new(SelectionInfo::default())),
        };

        let path = Path::from_string("/World/Mesh").unwrap();
        assert!(!scene.is_selected(&path));

        scene.add_selection(&path);
        assert!(scene.is_selected(&path));
        assert_eq!(scene.get_selected_paths().len(), 1);

        scene.clear_selection();
        assert!(!scene.is_selected(&path));
        assert!(scene.get_selected_paths().is_empty());
    }

    #[test]
    fn test_multiple_selections() {
        let mut scene = SelectionSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            selection_info: Arc::new(RwLock::new(SelectionInfo::default())),
        };

        scene.add_selection(&Path::from_string("/A").unwrap());
        scene.add_selection(&Path::from_string("/B").unwrap());
        scene.add_selection(&Path::from_string("/C").unwrap());

        assert_eq!(scene.get_selected_paths().len(), 3);
    }

    #[test]
    fn test_delete_prefix_on_remove() {
        let info = Arc::new(RwLock::new(SelectionInfo::default()));

        {
            let mut w = info.write();
            w.prim_to_selections.insert(
                Path::from_string("/World/A").unwrap(),
                SelectionEntry { count: 1 },
            );
            w.prim_to_selections.insert(
                Path::from_string("/World/B").unwrap(),
                SelectionEntry { count: 1 },
            );
            w.prim_to_selections.insert(
                Path::from_string("/Other").unwrap(),
                SelectionEntry { count: 1 },
            );
        }

        // Delete prefix /World should remove /World/A and /World/B
        info.write()
            .delete_prefix(&Path::from_string("/World").unwrap());

        let r = info.read();
        assert_eq!(r.prim_to_selections.len(), 1);
        assert!(
            r.prim_to_selections
                .contains_key(&Path::from_string("/Other").unwrap())
        );
    }

    #[test]
    fn test_add_selection_increments_count() {
        let mut scene = SelectionSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            selection_info: Arc::new(RwLock::new(SelectionInfo::default())),
        };

        let path = Path::from_string("/A").unwrap();
        scene.add_selection(&path);
        scene.add_selection(&path);

        let info = scene.selection_info.read();
        assert_eq!(info.prim_to_selections[&path].count, 2);
    }
}
