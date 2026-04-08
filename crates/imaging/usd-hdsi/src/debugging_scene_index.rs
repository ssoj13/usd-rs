//! Debugging scene index.
//!
//! A filtering scene index that checks for certain inconsistencies (without
//! transforming the scene) in its input scene.
//! For example, it will report if the input scene's GetPrim(/foo) returns a
//! prim type different from a previous call to GetPrim(/foo) even though the
//! input scene sent no related prims added or removed notice.
//!
//! Port of pxr/imaging/hdsi/debuggingSceneIndex.

use parking_lot::RwLock;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use usd_hd::data_source::{HdContainerDataSourceHandle, HdDataSourceBaseHandle};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// Policy constants (from C++)
const ALLOW_PROPERTY_PATHS: bool = true;
const IMPLICITLY_ADDED_ANCESTORS_HAVE_EMPTY_TYPE: bool = true;

/// Per-prim info for consistency tracking.
#[derive(Clone, Debug)]
struct PrimInfo {
    /// Does prim exist in namespace?
    exists_in_namespace: Option<bool>,
    /// Do we know all children of this prim?
    all_children_known: bool,
    /// Prim type if known.
    prim_type: Option<TfToken>,
    /// Does this prim have a non-null data source?
    has_data_source: Option<bool>,
}

impl Default for PrimInfo {
    fn default() -> Self {
        Self {
            exists_in_namespace: None,
            all_children_known: false,
            prim_type: None,
            has_data_source: None,
        }
    }
}

/// Ancestors iterator (includes "/").
struct Ancestors {
    path: Option<SdfPath>,
}

impl Ancestors {
    fn new(path: &SdfPath) -> Self {
        Self {
            path: if path.is_empty() {
                None
            } else {
                Some(path.clone())
            },
        }
    }
}

impl Iterator for Ancestors {
    type Item = SdfPath;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.path.take()?;
        let result = current.clone();
        if current.is_absolute_root_path() {
            self.path = None;
        } else {
            self.path = Some(current.get_parent_path());
        }
        Some(result)
    }
}

fn emit_message(message: &str) {
    eprintln!("[HdsiDebuggingSceneIndex] {}", message);
}

fn emit_error(message: &str) {
    emit_message(&format!("ERROR: {}", message));
}

fn data_source_string(has_data_source: bool) -> &'static str {
    if has_data_source {
        "non-null data source"
    } else {
        "null data source"
    }
}

fn is_valid_prim_path(prim_path: &SdfPath) -> bool {
    if prim_path.is_absolute_root_path() {
        return true;
    }
    if prim_path.is_prim_path() {
        return true;
    }
    if ALLOW_PROPERTY_PATHS && prim_path.is_property_path() {
        return true;
    }
    false
}

fn mark_prim_as_existing_in_namespace(
    prims: &mut BTreeMap<SdfPath, PrimInfo>,
    callsite: &str,
    prim_path: &SdfPath,
    prim_type: Option<TfToken>,
    has_data_source: Option<bool>,
) {
    let mut level = 0usize;
    let mut child_existed_in_namespace: Option<bool> = None;

    for ancestor in Ancestors::new(prim_path) {
        let prim_info = prims.entry(ancestor.clone()).or_default();

        let existed_in_namespace = prim_info.exists_in_namespace.replace(true);

        if existed_in_namespace == Some(false) {
            emit_error(&format!(
                "{}({}) returned non-trivial result even though the prim at {} was established to not exist in namespace.",
                callsite,
                prim_path.as_str(),
                ancestor.as_str()
            ));
        }

        if level == 0 {
            if let Some(ref pt) = prim_type {
                if let Some(ref existing) = prim_info.prim_type {
                    if existing != pt {
                        emit_error(&format!(
                            "{}({}) returned prim type {} even though the prim was established to be of type {}.",
                            callsite,
                            prim_path.as_str(),
                            pt.as_str(),
                            existing.as_str()
                        ));
                    }
                }
                prim_info.prim_type = Some(pt.clone());
            }

            if let Some(has_ds) = has_data_source {
                if let Some(existing) = prim_info.has_data_source {
                    if existing != has_ds {
                        emit_error(&format!(
                            "{}({}) returned {} even though the prim was established to have a {}.",
                            callsite,
                            prim_path.as_str(),
                            data_source_string(has_ds),
                            data_source_string(existing)
                        ));
                    }
                }
                prim_info.has_data_source = Some(has_ds);
            }
        } else if prim_info.all_children_known && child_existed_in_namespace != Some(true) {
            emit_error(&format!(
                "{}({}) returned a non-trivial result even though prim {} does not have a corresponding child.",
                callsite,
                prim_path.as_str(),
                ancestor.as_str()
            ));
        }

        child_existed_in_namespace = existed_in_namespace;
        level += 1;
    }
}

fn mark_prim_as_non_existing_in_namespace(
    prims: &mut BTreeMap<SdfPath, PrimInfo>,
    prim_path: &SdfPath,
) {
    prims.insert(
        prim_path.clone(),
        PrimInfo {
            exists_in_namespace: Some(prim_path.is_absolute_root_path()),
            all_children_known: true,
            prim_type: None,
            has_data_source: None,
        },
    );

    // Delete all descendants.
    let keys_to_remove: Vec<SdfPath> = prims
        .keys()
        .filter(|k| *k != prim_path && k.has_prefix(prim_path))
        .cloned()
        .collect();
    for k in keys_to_remove {
        prims.remove(&k);
    }
}

/// Debugging scene index that checks for inconsistencies in the input scene.
pub struct HdsiDebuggingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    prims: Mutex<BTreeMap<SdfPath, PrimInfo>>,
}

impl HdsiDebuggingSceneIndex {
    /// Creates a new debugging scene index.
    ///
    /// # Arguments
    /// * `input_scene` - Input scene to wrap
    /// * `input_args` - Optional input arguments (for API parity with C++)
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let display_name = input_scene.read().get_display_name();

        let mut prims = BTreeMap::new();
        prims.insert(
            SdfPath::absolute_root(),
            PrimInfo {
                exists_in_namespace: Some(true),
                all_children_known: false,
                prim_type: None,
                has_data_source: None,
            },
        );

        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            prims: Mutex::new(prims),
        }));

        emit_message(&format!(
            "Instantiated for '{}' of type 'HdSceneIndexBase'.",
            display_name
        ));

        // input_args currently unused but accepted for API parity
        let _ = input_args;

        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene
                .read()
                .add_observer(Arc::new(filtering_observer));
        }
        observer
    }
}

impl HdSceneIndexBase for HdsiDebuggingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let prim = match self.base.get_input_scene() {
            Some(input) => si_ref(&input).get_prim(prim_path),
            None => HdSceneIndexPrim::default(),
        };

        if !prim_path.is_absolute_path() {
            emit_error(&format!(
                "GetPrim({}) was called with relative path.",
                prim_path.as_str()
            ));
            return prim;
        }
        if !is_valid_prim_path(prim_path) {
            emit_error(&format!(
                "GetPrim({}) was called with non-prim/property path.",
                prim_path.as_str()
            ));
            return prim;
        }

        let exists = !prim.prim_type.is_empty() || prim.data_source.is_some();

        {
            let mut prims = self.prims.lock().expect("prims mutex");
            if exists {
                mark_prim_as_existing_in_namespace(
                    &mut prims,
                    "GetPrim",
                    prim_path,
                    Some(prim.prim_type.clone()),
                    Some(prim.data_source.is_some()),
                );
            } else {
                if let Some(prim_info) = prims.get(prim_path) {
                    if let Some(ref pt) = prim_info.prim_type {
                        if !pt.is_empty() {
                            emit_error(&format!(
                                "GetPrim({}) returned a trivial result even though the prim was previously established of type {}.",
                                prim_path.as_str(),
                                pt.as_str()
                            ));
                        }
                    }
                    if prim_info.has_data_source == Some(true) {
                        emit_error(&format!(
                            "GetPrim({}) returned a trivial result even though the prim was previously established to have a non-null data source.",
                            prim_path.as_str()
                        ));
                    }
                }
            }
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let child_prim_paths = match self.base.get_input_scene() {
            Some(input) => si_ref(&input).get_child_prim_paths(prim_path),
            None => Vec::new(),
        };

        if !prim_path.is_absolute_path() {
            emit_error(&format!(
                "GetChildPrimPaths({}) was called with relative path.",
                prim_path.as_str()
            ));
            return child_prim_paths;
        }
        if !is_valid_prim_path(prim_path) {
            emit_error(&format!(
                "GetChildPrimPaths({}) was called with non-prim/property path.",
                prim_path.as_str()
            ));
            return child_prim_paths;
        }

        for child_prim_path in &child_prim_paths {
            if !child_prim_path.is_absolute_path() {
                emit_error(&format!(
                    "GetChildPrimPaths({}) returned non-absolute path {}.",
                    prim_path.as_str(),
                    child_prim_path.as_str()
                ));
            }
            if !is_valid_prim_path(child_prim_path) {
                emit_error(&format!(
                    "GetChildPrimPaths({}) returned non-prim/property path {}.",
                    prim_path.as_str(),
                    child_prim_path.as_str()
                ));
            }
            if child_prim_path.get_parent_path() != *prim_path {
                emit_error(&format!(
                    "GetChildPrimPaths({}) returned non-child path {}.",
                    prim_path.as_str(),
                    child_prim_path.as_str()
                ));
            }
        }

        let exists_in_namespace = !child_prim_paths.is_empty();
        let child_prim_path_set: HashSet<&SdfPath> = child_prim_paths.iter().collect();

        {
            let mut prims = self.prims.lock().expect("prims mutex");

            // Check that every stored immediate child with existsInNamespace is in GetChildPrimPaths.
            for (path, prim_info) in prims.range(prim_path.clone()..) {
                if !path.has_prefix(prim_path) {
                    break;
                }
                if path.get_path_element_count() != prim_path.get_path_element_count() + 1 {
                    continue;
                }
                if prim_info.exists_in_namespace == Some(true) {
                    if !child_prim_path_set.contains(path) {
                        emit_error(&format!(
                            "GetChildPrimPaths({}) does not include {} even though it was established to exist.",
                            prim_path.as_str(),
                            path.as_str()
                        ));
                    }
                }
            }

            let prim_info = prims.entry(prim_path.clone()).or_default();
            let all_children_known = std::mem::replace(&mut prim_info.all_children_known, true);

            for child_prim_path in &child_prim_paths {
                let child_prim_info = prims.entry(child_prim_path.clone()).or_default();
                let child_exists_in_namespace = child_prim_info.exists_in_namespace.replace(true);

                if child_exists_in_namespace == Some(false) {
                    emit_error(&format!(
                        "GetChildPrimPaths({}) includes {} even though the prim was established to not exist.",
                        prim_path.as_str(),
                        child_prim_path.as_str()
                    ));
                } else if all_children_known && child_exists_in_namespace != Some(true) {
                    emit_error(&format!(
                        "GetChildPrimPaths({}) includes {} even though the prim was not included in a previous call to GetChildPrimPaths or its parent was deleted without it being re-added.",
                        prim_path.as_str(),
                        child_prim_path.as_str()
                    ));
                }
            }

            if exists_in_namespace {
                mark_prim_as_existing_in_namespace(
                    &mut prims,
                    "GetChildPrimPaths",
                    prim_path,
                    None,
                    None,
                );
            }
        }

        child_prim_paths
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiDebuggingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiDebuggingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        {
            let mut prims = self.prims.lock().expect("prims mutex");

            for entry in entries {
                if !entry.prim_path.is_absolute_path() {
                    emit_error(&format!(
                        "AddedPrimsEntry with relative path {}.",
                        entry.prim_path.as_str()
                    ));
                    continue;
                }
                if !is_valid_prim_path(&entry.prim_path) {
                    emit_error(&format!(
                        "AddedPrimsEntry with non-prim/property path {}.",
                        entry.prim_path.as_str()
                    ));
                    continue;
                }

                let mut level = 0;
                for ancestor in Ancestors::new(&entry.prim_path) {
                    let prim_info = prims.entry(ancestor).or_default();

                    let existed_in_namespace = prim_info.exists_in_namespace.replace(true);

                    if level == 0 {
                        prim_info.prim_type = Some(entry.prim_type.clone());
                    } else if IMPLICITLY_ADDED_ANCESTORS_HAVE_EMPTY_TYPE {
                        if existed_in_namespace == Some(false) {
                            prim_info.prim_type = Some(TfToken::empty());
                        }
                    }

                    level += 1;
                }
            }
        }

        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        {
            let mut prims = self.prims.lock().expect("prims mutex");

            for entry in entries {
                if !entry.prim_path.is_absolute_path() {
                    emit_error(&format!(
                        "RemovedPrimsEntry with relative path {}.",
                        entry.prim_path.as_str()
                    ));
                    continue;
                }
                if !is_valid_prim_path(&entry.prim_path) {
                    emit_error(&format!(
                        "RemovedPrimsEntry with non-prim/property path {}.",
                        entry.prim_path.as_str()
                    ));
                    continue;
                }

                mark_prim_as_non_existing_in_namespace(&mut prims, &entry.prim_path);
            }
        }

        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        {
            let _prims = self.prims.lock().expect("prims mutex");

            for entry in entries {
                if !entry.prim_path.is_absolute_path() {
                    emit_error(&format!(
                        "DirtiedPrimsEntry with relative path {}.",
                        entry.prim_path.as_str()
                    ));
                    continue;
                }
                if !is_valid_prim_path(&entry.prim_path) {
                    emit_error(&format!(
                        "DirtiedPrimsEntry with non-prim/property path {}.",
                        entry.prim_path.as_str()
                    ));
                    continue;
                }
            }
        }

        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        {
            let _prims = self.prims.lock().expect("prims mutex");

            for entry in entries {
                if !entry.old_prim_path.is_absolute_path() {
                    emit_error(&format!(
                        "RenamedPrimsEntry with relative old path {}.",
                        entry.old_prim_path.as_str()
                    ));
                    continue;
                }
                if !is_valid_prim_path(&entry.old_prim_path) {
                    emit_error(&format!(
                        "RenamedPrimsEntry with non-prim/property old path {}.",
                        entry.old_prim_path.as_str()
                    ));
                    continue;
                }
                if !entry.new_prim_path.is_absolute_path() {
                    emit_error(&format!(
                        "RenamedPrimsEntry with relative new path {}.",
                        entry.new_prim_path.as_str()
                    ));
                    continue;
                }
                if !is_valid_prim_path(&entry.new_prim_path) {
                    emit_error(&format!(
                        "RenamedPrimsEntry with non-prim/property new path {}.",
                        entry.new_prim_path.as_str()
                    ));
                    continue;
                }
            }
        }

        if !entries.is_empty() {
            emit_message(
                "Received RenamedPrimEntries but HdsiDebuggingSceneIndex does not support it (yet).",
            );
        }

        self.base.forward_prims_renamed(self, entries);
    }
}
