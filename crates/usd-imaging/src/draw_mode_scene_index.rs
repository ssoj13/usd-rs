#![allow(dead_code)]
//! DrawModeSceneIndex - Draw mode scene index for UsdImaging.
//!
//! Port of pxr/usdImaging/usdImaging/drawModeSceneIndex.cpp.
//!
//! A scene index that replaces geometry based on draw mode. Inspects prims
//! for `applyDrawMode` and `drawMode` values from GeomModelSchema. When a
//! non-default draw mode is active, the prim and all its descendants are
//! replaced by stand-in geometry (bounds wireframe box, cards, origin axes).

use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::data_source::HdDataSourceLocator;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
    wire_filter_to_input,
};
use usd_sdf::Path;
use usd_tf::Token;

use crate::draw_mode_standin::{self, DrawModeStandinHandle};

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static DEFAULT: LazyLock<Token> = LazyLock::new(|| Token::new("default"));
    pub static DRAW_MODE: LazyLock<Token> = LazyLock::new(|| Token::new("drawMode"));
    pub static APPLY_DRAW_MODE: LazyLock<Token> = LazyLock::new(|| Token::new("applyDrawMode"));
    pub static GEOM_MODEL: LazyLock<Token> = LazyLock::new(|| Token::new("geomModel"));
}

// ---------------------------------------------------------------------------
// _GetDrawMode - resolve draw mode from prim data source
// ---------------------------------------------------------------------------

/// Resolve draw mode for a prim from input scene index.
/// Default draw mode is expressed by either the empty token or "default".
///
/// Port of C++ `_GetDrawMode`.
fn get_draw_mode(prim: &HdSceneIndexPrim) -> Token {
    use usd_hd::data_source::cast_to_container;

    let ds = match &prim.data_source {
        Some(ds) => ds,
        None => return Token::empty(),
    };

    // Look up geomModel container
    let geom_model_base = match ds.get(&tokens::GEOM_MODEL) {
        Some(gm) => gm,
        None => return Token::empty(),
    };

    // Cast to container data source
    let gm_container = match cast_to_container(&geom_model_base) {
        Some(c) => c,
        None => return Token::empty(),
    };

    // Check applyDrawMode flag
    if let Some(apply_ds) = gm_container.get(&tokens::APPLY_DRAW_MODE) {
        if let Some(sampled) = apply_ds.as_sampled() {
            let val = sampled.get_value(0.0);
            if let Some(b) = val.get::<bool>() {
                if !b {
                    return Token::empty();
                }
            }
        }
    } else {
        return Token::empty();
    }

    // Get drawMode token
    if let Some(mode_ds) = gm_container.get(&tokens::DRAW_MODE) {
        if let Some(sampled) = mode_ds.as_sampled() {
            let val = sampled.get_value(0.0);
            if let Some(t) = val.get::<Token>() {
                return t.clone();
            }
        }
    }

    Token::empty()
}

/// Check if a draw mode is non-default (i.e. needs standin geometry).
fn is_non_default_draw_mode(mode: &Token) -> bool {
    !mode.is_empty() && mode != &*tokens::DEFAULT
}

/// Factory: create a standin if the mode is non-default.
fn get_draw_mode_standin(
    mode: &Token,
    path: &Path,
    prim_source: &Option<HdContainerDataSourceHandle>,
) -> Option<DrawModeStandinHandle> {
    if !is_non_default_draw_mode(mode) {
        return None;
    }
    draw_mode_standin::create_standin(mode, path, prim_source)
}

// ---------------------------------------------------------------------------
// Efficient prefix lookup in BTreeMap
// ---------------------------------------------------------------------------

/// Find entry in a BTreeMap<Path, V> where key is a prefix of `path`.
/// Port of C++ `_FindPrefixOfPath`.
fn find_prefix_of_path<'a, V>(
    container: &'a BTreeMap<Path, V>,
    path: &Path,
) -> Option<(&'a Path, &'a V)> {
    // Check exact match first
    if let Some((k, v)) = container.get_key_value(path) {
        return Some((k, v));
    }

    // Use lower_bound equivalent: the entry before `path` in sorted order
    // might be its prefix
    let mut it = container.range(..=path.clone());
    if let Some((key, val)) = it.next_back() {
        if path.has_prefix(key) {
            return Some((key, val));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// DrawModeSceneIndex
// ---------------------------------------------------------------------------

/// A scene index replacing geometry based on draw mode.
///
/// Port of C++ `UsdImagingDrawModeSceneIndex`.
///
/// Inspects a prim's values for drawMode and applyDrawMode.
/// If the drawMode is valid, not default, and applyDrawMode is true,
/// the prim and all its descendants are replaced by stand-in geometry.
pub struct DrawModeSceneIndex {
    /// Base filtering scene index
    base: HdSingleInputFilteringSceneIndexBase,
    /// Input args for configuration
    input_args: Option<HdContainerDataSourceHandle>,
    /// Map of paths to draw mode stand-ins
    prims: Mutex<BTreeMap<Path, DrawModeStandinHandle>>,
}

impl std::fmt::Debug for DrawModeSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrawModeSceneIndex")
            .field(
                "standin_count",
                &self.prims.lock().expect("Lock poisoned").len(),
            )
            .finish()
    }
}

impl DrawModeSceneIndex {
    /// Creates a new draw mode scene index.
    ///
    /// Recursively scans input scene for prims with non-default draw modes.
    pub fn new(
        input: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let si = Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input.clone())),
            input_args,
            prims: Mutex::new(BTreeMap::new()),
        };

        // Initial scan: recurse from root to find prims with draw modes
        let root = Path::absolute_root();
        {
            let prim = si_ref(&input).get_prim(&root);
            let mode = get_draw_mode(&prim);
            // Use iterative approach to avoid lock issues
            si.recurse_prims_iterative(&mode, &root, &prim, &input);
        }

        let result = Arc::new(RwLock::new(si));
        wire_filter_to_input(&result, &input);
        result
    }

    /// Find standin for a prim or its ancestor.
    /// Port of C++ `_FindStandinForPrimOrAncestor`.
    fn find_standin_for_prim_or_ancestor(
        &self,
        path: &Path,
    ) -> Option<(DrawModeStandinHandle, bool)> {
        let prims = self.prims.lock().expect("Lock poisoned");
        if let Some((key, standin)) = find_prefix_of_path(&prims, path) {
            let is_descendant = path.get_path_element_count() > key.get_path_element_count();
            Some((standin.clone(), is_descendant))
        } else {
            None
        }
    }

    /// Delete all standins in a subtree.
    /// Port of C++ `_DeleteSubtree`.
    fn delete_subtree(&self, path: &Path) {
        let mut prims = self.prims.lock().expect("Lock poisoned");
        let keys: Vec<Path> = prims
            .range(path.clone()..)
            .take_while(|(k, _)| k.has_prefix(path))
            .map(|(k, _)| k.clone())
            .collect();
        for k in keys {
            prims.remove(&k);
        }
    }

    /// Iterative version of recurse_prims to avoid lock issues.
    /// Port of C++ `_RecursePrims`.
    fn recurse_prims_iterative(
        &self,
        _mode: &Token,
        root: &Path,
        _root_prim: &HdSceneIndexPrim,
        input: &HdSceneIndexHandle,
    ) {
        // Stack-based DFS
        let mut stack: Vec<(Path, Token)> = vec![(root.clone(), _mode.clone())];

        while let Some((path, mode)) = stack.pop() {
            let prim = si_ref(&input).get_prim(&path);

            if let Some(standin) = get_draw_mode_standin(&mode, &path, &prim.data_source) {
                // Non-default draw mode - store standin, don't recurse children
                self.prims
                    .lock()
                    .expect("Lock poisoned")
                    .insert(path.clone(), standin);
            } else {
                // Default draw mode - recurse to children
                let children = si_ref(&input).get_child_prim_paths(&path);
                for child_path in children.into_iter().rev() {
                    let child_prim = si_ref(&input).get_prim(&child_path);
                    let child_mode = get_draw_mode(&child_prim);
                    stack.push((child_path, child_mode));
                }
            }
        }
    }

    /// Recursive version with entries output (used during dirty processing).
    fn recurse_prims_with_entries(
        &self,
        mode: &Token,
        path: &Path,
        prim: &HdSceneIndexPrim,
        input: &HdSceneIndexHandle,
        entries: &mut Vec<AddedPrimEntry>,
    ) {
        if let Some(standin) = get_draw_mode_standin(mode, path, &prim.data_source) {
            standin.compute_prim_added_entries(entries);
            self.prims
                .lock()
                .expect("Lock poisoned")
                .insert(path.clone(), standin);
        } else {
            entries.push(AddedPrimEntry::new(path.clone(), prim.prim_type.clone()));
            let children = si_ref(&input).get_child_prim_paths(path);
            for child_path in &children {
                let (child_prim, child_mode) = {
                    let p = si_ref(&input).get_prim(child_path);
                    let m = get_draw_mode(&p);
                    (p, m)
                };
                self.recurse_prims_with_entries(
                    &child_mode,
                    child_path,
                    &child_prim,
                    input,
                    entries,
                );
            }
        }
    }

    /// Check if a path is affected by draw mode.
    pub fn is_path_affected(&self, path: &Path) -> bool {
        self.find_standin_for_prim_or_ancestor(path).is_some()
    }
}

/// Check if `path` is an immediate child of `parent_path`.
fn is_immediate_child_of(path: &Path, parent_path: &Path) -> bool {
    path.get_path_element_count() - parent_path.get_path_element_count() == 1
        && path.has_prefix(parent_path)
}

impl HdSceneIndexBase for DrawModeSceneIndex {
    /// Get prim - returns standin geometry if draw mode is active.
    /// Port of C++ `GetPrim`.
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        if let Some((standin, _)) = self.find_standin_for_prim_or_ancestor(prim_path) {
            return standin.get_prim(prim_path);
        }

        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_prim(prim_path);
        }
        HdSceneIndexPrim::default()
    }

    /// Get child prim paths - returns standin children if draw mode is active.
    /// Port of C++ `GetChildPrimPaths`.
    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        if let Some((standin, _)) = self.find_standin_for_prim_or_ancestor(prim_path) {
            // Return only immediate children of the queried path from standin
            return standin
                .get_prim_paths()
                .into_iter()
                .filter(|p| is_immediate_child_of(p, prim_path))
                .collect();
        }

        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
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
        "DrawModeSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for DrawModeSceneIndex {
    /// Handle PrimsAdded - check for draw mode changes on added prims.
    /// Port of C++ `_PrimsAdded`.
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let input = match self.base.get_input_scene() {
            Some(s) => s.clone(),
            None => {
                self.base.forward_prims_added(self, entries);
                return;
            }
        };

        let mut new_entries = Vec::new();
        let mut removed_entries = Vec::new();

        for entry in entries {
            let path = &entry.prim_path;

            // Suppress descendants of prims that already have a standin
            if let Some((_, true)) = self.find_standin_for_prim_or_ancestor(path) {
                continue;
            }

            // Get prim and check draw mode
            let prim = si_ref(&input).get_prim(path);
            let draw_mode = get_draw_mode(&prim);

            if let Some(standin) = get_draw_mode_standin(&draw_mode, path, &prim.data_source) {
                // Non-default draw mode - remove old subtree, add standin
                self.delete_subtree(path);
                removed_entries.push(RemovedPrimEntry::new(path.clone()));
                standin.compute_prim_added_entries(&mut new_entries);
                self.prims
                    .lock()
                    .expect("Lock poisoned")
                    .insert(path.clone(), standin);
            } else {
                // Default draw mode - forward as-is
                new_entries.push(entry.clone());
            }
        }

        if !removed_entries.is_empty() {
            self.base.forward_prims_removed(self, &removed_entries);
        }
        if !new_entries.is_empty() {
            self.base.forward_prims_added(self, &new_entries);
        }
    }

    /// Handle PrimsRemoved - clean up standins for removed prims.
    /// Port of C++ `_PrimsRemoved`.
    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if !self.prims.lock().expect("Lock poisoned").is_empty() {
            for entry in entries {
                self.delete_subtree(&entry.prim_path);
            }
        }
        self.base.forward_prims_removed(self, entries);
    }

    /// Handle PrimsDirtied - check for draw mode locator changes.
    /// Port of C++ `_PrimsDirtied`.
    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        // Locators that indicate draw mode changes
        let draw_mode_locator = HdDataSourceLocator::from_tokens_2(
            tokens::GEOM_MODEL.clone(),
            tokens::DRAW_MODE.clone(),
        );
        let apply_draw_mode_locator = HdDataSourceLocator::from_tokens_2(
            tokens::GEOM_MODEL.clone(),
            tokens::APPLY_DRAW_MODE.clone(),
        );

        // Collect paths whose draw mode might have changed
        let mut changed_paths: Vec<Path> = Vec::new();
        for entry in entries {
            if entry.dirty_locators.contains(&draw_mode_locator)
                || entry.dirty_locators.contains(&apply_draw_mode_locator)
            {
                changed_paths.push(entry.prim_path.clone());
            }
        }

        let mut removed_entries = Vec::new();
        let mut added_entries = Vec::new();

        if !changed_paths.is_empty() {
            let input = match self.base.get_input_scene() {
                Some(s) => s.clone(),
                None => {
                    self.base.forward_prims_dirtied(self, entries);
                    return;
                }
            };

            // Skip descendants of paths we've already processed
            let mut last_path = Path::empty();

            for path in &changed_paths {
                if !last_path.is_empty() && path.has_prefix(&last_path) {
                    continue;
                }
                last_path = Path::empty();

                // Suppress if ancestor has standin
                if let Some((_, true)) = self.find_standin_for_prim_or_ancestor(path) {
                    continue;
                }

                let prim = si_ref(&input).get_prim(path);
                let draw_mode = get_draw_mode(&prim);

                let existing = self.prims.lock().expect("Lock poisoned").get(path).cloned();

                match existing {
                    None => {
                        // Was default draw mode
                        if let Some(standin) =
                            get_draw_mode_standin(&draw_mode, path, &prim.data_source)
                        {
                            // Now non-default: remove old geometry, add standin
                            self.delete_subtree(path);
                            removed_entries.push(RemovedPrimEntry::new(path.clone()));
                            standin.compute_prim_added_entries(&mut added_entries);
                            self.prims
                                .lock()
                                .expect("Lock poisoned")
                                .insert(path.clone(), standin);
                            last_path = path.clone();
                        }
                    }
                    Some(old_standin) => {
                        if old_standin.get_draw_mode() != draw_mode {
                            // Draw mode changed (possibly back to default)
                            self.delete_subtree(path);
                            removed_entries.push(RemovedPrimEntry::new(path.clone()));

                            // Re-scan from this path
                            self.recurse_prims_with_entries(
                                &draw_mode,
                                path,
                                &prim,
                                &input,
                                &mut added_entries,
                            );
                            last_path = path.clone();
                        }
                    }
                }
            }
        }

        // Process non-draw-mode dirty locators
        let mut dirtied_entries = Vec::new();

        if self.prims.lock().expect("Lock poisoned").is_empty() {
            // No standins - fast path
            if !removed_entries.is_empty() {
                self.base.forward_prims_removed(self, &removed_entries);
            }
            if !added_entries.is_empty() {
                self.base.forward_prims_added(self, &added_entries);
            }
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        for entry in entries {
            let path = &entry.prim_path;
            match self.find_standin_for_prim_or_ancestor(path) {
                None => {
                    // Not under a standin - forward as-is
                    dirtied_entries.push(entry.clone());
                }
                Some((_, true)) => {
                    // Descendant of standin - suppress
                }
                Some((standin, false)) => {
                    // The standin prim itself is dirty
                    let mut needs_refresh = false;
                    standin.process_dirty_locators(
                        &entry.dirty_locators,
                        &mut dirtied_entries,
                        &mut needs_refresh,
                    );
                    if needs_refresh {
                        let input = match self.base.get_input_scene() {
                            Some(s) => s.clone(),
                            None => continue,
                        };
                        let prim = si_ref(&input).get_prim(path);
                        if let Some(new_standin) =
                            get_draw_mode_standin(&standin.get_draw_mode(), path, &prim.data_source)
                        {
                            removed_entries.push(RemovedPrimEntry::new(path.clone()));
                            new_standin.compute_prim_added_entries(&mut added_entries);
                            self.prims
                                .lock()
                                .expect("Lock poisoned")
                                .insert(path.clone(), new_standin);
                        }
                    }
                }
            }
        }

        if !removed_entries.is_empty() {
            self.base.forward_prims_removed(self, &removed_entries);
        }
        if !added_entries.is_empty() {
            self.base.forward_prims_added(self, &added_entries);
        }
        if !dirtied_entries.is_empty() {
            self.base.forward_prims_dirtied(self, &dirtied_entries);
        }
    }

    fn on_prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        // Convert rename to remove+add, matching C++
        let (removed, added) =
            usd_hd::scene_index::observer::convert_prims_renamed_to_removed_and_added(
                sender, entries,
            );
        if !removed.is_empty() {
            self.on_prims_removed(sender, &removed);
        }
        if !added.is_empty() {
            self.on_prims_added(sender, &added);
        }
    }
}

/// Handle type for DrawModeSceneIndex.
pub type DrawModeSceneIndexHandle = Arc<RwLock<DrawModeSceneIndex>>;

/// Creates a new draw mode scene index.
pub fn create_draw_mode_scene_index(
    input: HdSceneIndexHandle,
    input_args: Option<HdContainerDataSourceHandle>,
) -> DrawModeSceneIndexHandle {
    DrawModeSceneIndex::new(input, input_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_non_default_draw_mode() {
        assert!(!is_non_default_draw_mode(&Token::empty()));
        assert!(!is_non_default_draw_mode(&Token::new("default")));
        assert!(is_non_default_draw_mode(&Token::new("bounds")));
        assert!(is_non_default_draw_mode(&Token::new("cards")));
        assert!(is_non_default_draw_mode(&Token::new("origin")));
    }

    #[test]
    fn test_get_draw_mode_standin_default() {
        assert!(
            get_draw_mode_standin(
                &Token::new("default"),
                &Path::from_string("/Foo").unwrap(),
                &None
            )
            .is_none()
        );
    }

    #[test]
    fn test_get_draw_mode_standin_bounds() {
        let standin = get_draw_mode_standin(
            &Token::new("bounds"),
            &Path::from_string("/Foo").unwrap(),
            &None,
        );
        assert!(standin.is_some());
        assert_eq!(standin.unwrap().get_draw_mode().as_str(), "bounds");
    }

    #[test]
    fn test_get_draw_mode_standin_origin() {
        let standin = get_draw_mode_standin(
            &Token::new("origin"),
            &Path::from_string("/Foo").unwrap(),
            &None,
        );
        assert!(standin.is_some());
        assert_eq!(standin.unwrap().get_draw_mode().as_str(), "origin");
    }

    #[test]
    fn test_get_draw_mode_standin_cards() {
        let standin = get_draw_mode_standin(
            &Token::new("cards"),
            &Path::from_string("/Foo").unwrap(),
            &None,
        );
        assert!(standin.is_some());
        assert_eq!(standin.unwrap().get_draw_mode().as_str(), "cards");
    }

    #[test]
    fn test_find_prefix_of_path() {
        let mut map = BTreeMap::new();
        let standin = get_draw_mode_standin(
            &Token::new("bounds"),
            &Path::from_string("/World/Model").unwrap(),
            &None,
        )
        .unwrap();
        map.insert(Path::from_string("/World/Model").unwrap(), standin);

        // Exact match
        let result = find_prefix_of_path(&map, &Path::from_string("/World/Model").unwrap());
        assert!(result.is_some());

        // Descendant
        let result = find_prefix_of_path(&map, &Path::from_string("/World/Model/Child").unwrap());
        assert!(result.is_some());

        // Not under
        let result = find_prefix_of_path(&map, &Path::from_string("/Other").unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn test_is_immediate_child_of() {
        let parent = Path::from_string("/World").unwrap();
        let child = Path::from_string("/World/Model").unwrap();
        let grandchild = Path::from_string("/World/Model/Mesh").unwrap();

        assert!(is_immediate_child_of(&child, &parent));
        assert!(!is_immediate_child_of(&grandchild, &parent));
    }

    #[test]
    fn test_bounds_standin_added_entries() {
        let standin = get_draw_mode_standin(
            &Token::new("bounds"),
            &Path::from_string("/World/Model").unwrap(),
            &None,
        )
        .unwrap();
        let mut entries = Vec::new();
        standin.compute_prim_added_entries(&mut entries);
        // Bounds has root + boundsCurves child
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].prim_path.get_text(), "/World/Model");
        assert_eq!(entries[1].prim_path.get_text(), "/World/Model/boundsCurves");
        assert_eq!(entries[1].prim_type.as_str(), "basisCurves");
    }

    #[test]
    fn test_origin_standin_added_entries() {
        let standin = get_draw_mode_standin(
            &Token::new("origin"),
            &Path::from_string("/World/Model").unwrap(),
            &None,
        )
        .unwrap();
        let mut entries = Vec::new();
        standin.compute_prim_added_entries(&mut entries);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].prim_path.get_text(), "/World/Model/originCurves");
        assert_eq!(entries[1].prim_type.as_str(), "basisCurves");
    }

    #[test]
    fn test_cards_standin_added_entries() {
        let standin = get_draw_mode_standin(
            &Token::new("cards"),
            &Path::from_string("/World/Model").unwrap(),
            &None,
        )
        .unwrap();
        let mut entries = Vec::new();
        standin.compute_prim_added_entries(&mut entries);
        // Cards has root + cardsMesh (no textures = no materials/subsets)
        assert!(entries.len() >= 2);
        assert_eq!(entries[1].prim_path.get_text(), "/World/Model/cardsMesh");
        assert_eq!(entries[1].prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_delete_subtree() {
        let mut prims = BTreeMap::new();
        prims.insert(
            Path::from_string("/A").unwrap(),
            get_draw_mode_standin(
                &Token::new("bounds"),
                &Path::from_string("/A").unwrap(),
                &None,
            )
            .unwrap(),
        );
        prims.insert(
            Path::from_string("/A/B").unwrap(),
            get_draw_mode_standin(
                &Token::new("bounds"),
                &Path::from_string("/A/B").unwrap(),
                &None,
            )
            .unwrap(),
        );
        prims.insert(
            Path::from_string("/C").unwrap(),
            get_draw_mode_standin(
                &Token::new("bounds"),
                &Path::from_string("/C").unwrap(),
                &None,
            )
            .unwrap(),
        );

        // Delete /A subtree
        let path = Path::from_string("/A").unwrap();
        let keys: Vec<Path> = prims
            .range(path.clone()..)
            .take_while(|(k, _)| k.has_prefix(&path))
            .map(|(k, _)| k.clone())
            .collect();
        for k in keys {
            prims.remove(&k);
        }

        assert_eq!(prims.len(), 1);
        assert!(prims.contains_key(&Path::from_string("/C").unwrap()));
    }
}
