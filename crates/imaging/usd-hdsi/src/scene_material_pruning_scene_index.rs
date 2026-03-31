
//! Scene material pruning scene index.
//!
//! Port of pxr/imaging/hdsi/sceneMaterialPruningSceneIndex.
//!
//! When enabled, prunes material bindings from geometry (unless materialIsFinal)
//! and clears prim type for non-builtin materials.

use crate::utils;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedContainerDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{
    HdBuiltinMaterialSchema, HdLegacyDisplayStyleSchema, HdMaterialBindingsSchema,
    MATERIAL_BINDINGS,
};
use usd_hd::tokens;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

fn material_is_final(prim_source: &HdContainerDataSourceHandle) -> bool {
    let schema = HdLegacyDisplayStyleSchema::get_from_parent(prim_source);
    schema.get_material_is_final().unwrap_or(false)
}

fn prune_material(prim_source: &HdContainerDataSourceHandle) -> bool {
    let schema = HdBuiltinMaterialSchema::get_from_parent(prim_source);
    !schema.get_builtin_material().unwrap_or(false)
}

fn prune_material_binding(prim_source: &HdContainerDataSourceHandle) -> bool {
    let bindings = HdMaterialBindingsSchema::get_from_parent(prim_source);
    if !bindings.is_defined() {
        return false;
    }
    !material_is_final(prim_source)
}

/// Prim data source that filters out materialBindings when pruning is enabled.
#[derive(Clone)]
struct PrimDataSource {
    input: HdContainerDataSourceHandle,
    info_enabled: Arc<AtomicBool>,
}

impl fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrimDataSource").finish_non_exhaustive()
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrimDataSource {
            input: self.input.clone(),
            info_enabled: Arc::clone(&self.info_enabled),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name.as_str() == MATERIAL_BINDINGS.as_str()
            && self.info_enabled.load(Ordering::SeqCst)
            && !material_is_final(&self.input)
        {
            return None;
        }
        self.input.get(name)
    }
}

/// Scene index filter that prunes material bindings and material prims.
///
/// When enabled, removes material bindings from geometry (unless materialIsFinal)
/// and clears prim type for non-builtin materials.
pub struct HdsiSceneMaterialPruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    info_enabled: Arc<AtomicBool>,
}

impl HdsiSceneMaterialPruningSceneIndex {
    /// Creates a new material pruning scene index.
    ///
    /// # Arguments
    /// * `input_scene` - The scene index to filter
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let info_enabled = Arc::new(AtomicBool::new(false));
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            info_enabled: Arc::clone(&info_enabled),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    /// Returns whether material pruning is enabled.
    pub fn get_enabled(this: &Arc<RwLock<Self>>) -> bool {
        let guard = this.read();
        guard.info_enabled.load(Ordering::SeqCst)
    }

    /// Enables or disables material pruning.
    ///
    /// When the state changes, sends PrimsAdded for affected materials and
    /// PrimsDirtied for geometry with material bindings.
    pub fn set_enabled(this: &Arc<RwLock<Self>>, enabled: bool) {
        let input_handle = {
            let guard = this.write();
            let prev = guard.info_enabled.load(Ordering::SeqCst);
            if prev == enabled {
                return;
            }
            guard.info_enabled.store(enabled, Ordering::SeqCst);
            guard.base.get_input_scene().cloned()
        };

        if input_handle.is_none() {
            return;
        }
        let input = input_handle.unwrap();

        let root = SdfPath::absolute_root();
        let prim_paths = utils::collect_prim_paths(&input, &root);

        let mut added_entries = Vec::new();
        let mut dirtied_entries = Vec::new();
        let binding_locators = HdMaterialBindingsSchema::get_default_locator_set();

        {
            let guard = this.read();
            if !guard.base.base().is_observed() {
                return;
            }

            for prim_path in prim_paths {
                let prim = si_ref(&input).get_prim(&prim_path);

                let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
                if prim.prim_type == *tokens::SPRIM_MATERIAL {
                    if prune_material(prim.data_source.as_ref().unwrap_or(&empty)) {
                        added_entries.push(AddedPrimEntry {
                            prim_path,
                            prim_type: if enabled {
                                TfToken::default()
                            } else {
                                prim.prim_type
                            },
                            data_source: None,
                        });
                    }
                } else if prune_material_binding(prim.data_source.as_ref().unwrap_or(&empty)) {
                    dirtied_entries.push(DirtiedPrimEntry::new(
                        prim_path.clone(),
                        binding_locators.clone(),
                    ));
                }
            }
        }

        if !added_entries.is_empty() {
            let guard = this.read();
            let delegate = usd_hd::scene_index::base::SceneIndexDelegate(Arc::clone(this));
            let sender = &delegate as &dyn HdSceneIndexBase;
            guard.base.base().send_prims_added(sender, &added_entries);
        }
        if !dirtied_entries.is_empty() {
            let guard = this.read();
            let delegate = usd_hd::scene_index::base::SceneIndexDelegate(Arc::clone(this));
            let sender = &delegate as &dyn HdSceneIndexBase;
            guard.base.base().send_prims_dirtied(sender, &dirtied_entries);
        }
    }
}

impl HdSceneIndexBase for HdsiSceneMaterialPruningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            return HdSceneIndexPrim::default();
        };

        let prim_ds = prim.data_source.as_ref().unwrap_or(&empty);

        if prim.prim_type == *tokens::SPRIM_MATERIAL {
            if self.info_enabled.load(Ordering::SeqCst) && prune_material(prim_ds) {
                prim.prim_type = TfToken::default();
                prim.data_source = None;
            }
        } else if prim.data_source.is_some() {
            prim.data_source = Some(Arc::new(PrimDataSource {
                input: prim.data_source.clone().unwrap(),
                info_enabled: Arc::clone(&self.info_enabled),
            }) as HdContainerDataSourceHandle);
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
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

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiSceneMaterialPruningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiSceneMaterialPruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        if !self.info_enabled.load(Ordering::SeqCst) {
            self.base.forward_prims_added(self, entries);
            return;
        }

        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => {
                self.base.forward_prims_added(self, entries);
                return;
            }
        };

        let mut new_entries: Vec<AddedPrimEntry> = entries.to_vec();
        for entry in &mut new_entries {
            if entry.prim_type == *tokens::SPRIM_MATERIAL {
                let prim = si_ref(&input).get_prim(&entry.prim_path);
                let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
                let prim_ds = prim.data_source.as_ref().unwrap_or(&empty);
                if prune_material(prim_ds) {
                    entry.prim_type = TfToken::default();
                }
            }
        }
        self.base.forward_prims_added(self, &new_entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        if !self.info_enabled.load(Ordering::SeqCst) {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => {
                self.base.forward_prims_dirtied(self, entries);
                return;
            }
        };

        let material_is_final_locator = HdLegacyDisplayStyleSchema::get_material_is_final_locator();
        let builtin_material_locator = HdBuiltinMaterialSchema::get_builtin_material_locator();

        let mut added_entries = Vec::new();
        let mut new_entries: Vec<DirtiedPrimEntry> = entries.to_vec();

        for (i, entry) in entries.iter().enumerate() {
            if entry
                .dirty_locators
                .intersects_locator(&builtin_material_locator)
            {
                let prim = si_ref(&input).get_prim(&entry.prim_path);
                let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
                if prim.prim_type == *tokens::SPRIM_MATERIAL {
                    let prim_ds = prim.data_source.as_ref().unwrap_or(&empty);
                    if prune_material(prim_ds) {
                        added_entries.push(AddedPrimEntry {
                            prim_path: entry.prim_path.clone(),
                            prim_type: TfToken::default(),
                            data_source: None,
                        });
                    } else {
                        added_entries.push(AddedPrimEntry {
                            prim_path: entry.prim_path.clone(),
                            prim_type: prim.prim_type.clone(),
                            data_source: None,
                        });
                    }
                }
            }
            if entry
                .dirty_locators
                .intersects_locator(&material_is_final_locator)
            {
                new_entries[i]
                    .dirty_locators
                    .insert(HdMaterialBindingsSchema::get_default_locator().clone());
            }
        }

        if !added_entries.is_empty() {
            self.base.base().send_prims_added(self, &added_entries);
        }
        self.base.forward_prims_dirtied(self, &new_entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
