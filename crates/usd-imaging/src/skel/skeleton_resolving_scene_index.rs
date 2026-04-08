//! SkeletonResolvingSceneIndex - Resolves skeleton prims to mesh guides.
//!
//! Port of pxr/usdImaging/usdSkelImaging/skeletonResolvingSceneIndex.h/cpp
//!
//! For each Skeleton prim: overlays ResolvedSkeletonSchema + guide mesh data and
//! changes prim type from skeleton to mesh.

use super::animation_schema::AnimationSchema;
use super::data_source_resolved_skeleton_prim::DataSourceResolvedSkeletonPrim;
use super::tokens::PRIM_TYPE_TOKENS;
use super::xform_resolver::DataSourceXformResolver;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use usd_hd::data_source::{HdDataSourceLocatorSet, HdOverlayContainerDataSource};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref, wire_filter_to_input,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

fn mesh_prim_type() -> Token {
    Token::new("mesh")
}

fn lookup<'a>(
    map: &'a HashMap<SdfPath, HashSet<SdfPath>>,
    key: &'a SdfPath,
) -> impl Iterator<Item = &'a SdfPath> {
    map.get(key).into_iter().flat_map(|set| set.iter())
}

fn populate_from_dependencies(
    dependencies: &HashMap<SdfPath, HashSet<SdfPath>>,
    prefix: &SdfPath,
    prim_paths: &mut HashSet<SdfPath>,
) {
    for (dependency_path, skeleton_paths) in dependencies {
        if dependency_path.has_prefix(prefix) {
            prim_paths.extend(skeleton_paths.iter().cloned());
        }
    }
}

fn remove_dependency(map: &mut HashMap<SdfPath, HashSet<SdfPath>>, key: &SdfPath, value: &SdfPath) {
    let should_remove_key = if let Some(set) = map.get_mut(key) {
        set.remove(value);
        set.is_empty()
    } else {
        false
    };

    if should_remove_key {
        map.remove(key);
    }
}

struct SkeletonResolvingState {
    path_to_resolved_skeleton: HashMap<SdfPath, Arc<DataSourceResolvedSkeletonPrim>>,
    skel_anim_path_to_skeleton_paths: HashMap<SdfPath, HashSet<SdfPath>>,
    instancer_path_to_skeleton_paths: HashMap<SdfPath, HashSet<SdfPath>>,
}

impl SkeletonResolvingState {
    fn new() -> Self {
        Self {
            path_to_resolved_skeleton: HashMap::new(),
            skel_anim_path_to_skeleton_paths: HashMap::new(),
            instancer_path_to_skeleton_paths: HashMap::new(),
        }
    }
}

fn add_dependencies_for_resolved_skeleton_state(
    state: &mut SkeletonResolvingState,
    skeleton_path: &SdfPath,
    resolved_skeleton: &Arc<DataSourceResolvedSkeletonPrim>,
) {
    for anim_source in resolved_skeleton.get_resolved_animation_sources() {
        if !anim_source.is_empty() {
            state
                .skel_anim_path_to_skeleton_paths
                .entry(anim_source)
                .or_default()
                .insert(skeleton_path.clone());
        }
    }

    for instancer_path in resolved_skeleton.get_instancer_paths() {
        state
            .instancer_path_to_skeleton_paths
            .entry(instancer_path.clone())
            .or_default()
            .insert(skeleton_path.clone());
    }
}

fn remove_dependencies_for_resolved_skeleton_state(
    state: &mut SkeletonResolvingState,
    skeleton_path: &SdfPath,
    resolved_skeleton: &Arc<DataSourceResolvedSkeletonPrim>,
) {
    for anim_source in resolved_skeleton.get_resolved_animation_sources() {
        if !anim_source.is_empty() {
            remove_dependency(
                &mut state.skel_anim_path_to_skeleton_paths,
                &anim_source,
                skeleton_path,
            );
        }
    }

    for instancer_path in resolved_skeleton.get_instancer_paths() {
        remove_dependency(
            &mut state.instancer_path_to_skeleton_paths,
            instancer_path,
            skeleton_path,
        );
    }
}

/// Scene index that resolves Skeleton prims to mesh guides with ResolvedSkeletonSchema.
pub struct SkeletonResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    state: Mutex<SkeletonResolvingState>,
}

impl std::fmt::Debug for SkeletonResolvingSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkeletonResolvingSceneIndex")
            .field(
                "resolved_count",
                &self
                    .state
                    .lock()
                    .expect("Lock poisoned")
                    .path_to_resolved_skeleton
                    .len(),
            )
            .finish()
    }
}

impl SkeletonResolvingSceneIndex {
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            state: Mutex::new(SkeletonResolvingState::new()),
        }));
        wire_filter_to_input(&this, &input_scene);

        let input = this.read().base.get_input_scene().cloned();
        if let Some(input) = input {
            let input_locked = input.read();
            let guard = this.read();
            for path in Self::collect_prim_paths(&*input_locked, &SdfPath::absolute_root()) {
                let _ = guard.add_resolved_skeleton(input.clone(), &path);
            }
        }

        this
    }

    fn collect_prim_paths(scene: &dyn HdSceneIndexBase, root: &SdfPath) -> Vec<SdfPath> {
        let mut result = Vec::new();
        for child in scene.get_child_prim_paths(root) {
            result.push(child.clone());
            result.extend(Self::collect_prim_paths(scene, &child));
        }
        result
    }

    fn add_resolved_skeleton(&self, input_handle: HdSceneIndexHandle, path: &SdfPath) -> bool {
        let prim = si_ref(&input_handle).get_prim(path);
        let Some(prim_source) = prim.data_source else {
            return false;
        };
        if prim.prim_type != PRIM_TYPE_TOKENS.skeleton {
            return false;
        }

        let ds = DataSourceResolvedSkeletonPrim::new(input_handle, path.clone(), prim_source);
        let mut state = self.state.lock().expect("Lock poisoned");
        add_dependencies_for_resolved_skeleton_state(&mut state, path, &ds);
        state.path_to_resolved_skeleton.insert(path.clone(), ds);
        true
    }

    fn remove_resolved_skeleton(&self, path: &SdfPath) -> bool {
        let mut state = self.state.lock().expect("Lock poisoned");
        let Some(resolved) = state.path_to_resolved_skeleton.remove(path) else {
            return false;
        };
        remove_dependencies_for_resolved_skeleton_state(&mut state, path, &resolved);
        true
    }

    fn refresh_resolved_skeleton_data_source(
        &self,
        input_handle: HdSceneIndexHandle,
        skeleton_path: &SdfPath,
    ) {
        let old_entry = self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_skeleton
            .get(skeleton_path)
            .cloned();
        let Some(old_entry) = old_entry else {
            return;
        };

        let prim = si_ref(&input_handle).get_prim(skeleton_path);

        if prim.prim_type != PRIM_TYPE_TOKENS.skeleton {
            let mut state = self.state.lock().expect("Lock poisoned");
            remove_dependencies_for_resolved_skeleton_state(&mut state, skeleton_path, &old_entry);
            state.path_to_resolved_skeleton.remove(skeleton_path);
            return;
        }
        let Some(prim_source) = prim.data_source else {
            let mut state = self.state.lock().expect("Lock poisoned");
            remove_dependencies_for_resolved_skeleton_state(&mut state, skeleton_path, &old_entry);
            state.path_to_resolved_skeleton.remove(skeleton_path);
            return;
        };

        let entry =
            DataSourceResolvedSkeletonPrim::new(input_handle, skeleton_path.clone(), prim_source);
        let mut state = self.state.lock().expect("Lock poisoned");
        remove_dependencies_for_resolved_skeleton_state(&mut state, skeleton_path, &old_entry);
        add_dependencies_for_resolved_skeleton_state(&mut state, skeleton_path, &entry);
        state
            .path_to_resolved_skeleton
            .insert(skeleton_path.clone(), entry);
    }

    fn process_dirty_locators(
        &self,
        input_handle: HdSceneIndexHandle,
        skel_path: &SdfPath,
        dirtied_prim_type: &Token,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: &mut Vec<DirtiedPrimEntry>,
    ) {
        let resolved = self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_skeleton
            .get(skel_path)
            .cloned();
        let Some(resolved) = resolved else {
            return;
        };

        if !resolved.process_dirty_locators(dirtied_prim_type, dirty_locators, Some(entries)) {
            return;
        }

        self.remove_resolved_skeleton(skel_path);
        let _ = self.add_resolved_skeleton(input_handle, skel_path);
        entries.push(DirtiedPrimEntry::new(
            skel_path.clone(),
            HdDataSourceLocatorSet::universal(),
        ));
    }
}

impl HdSceneIndexBase for SkeletonResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };
        let input_locked = input.read();

        let prim = input_locked.get_prim(prim_path);
        if prim.prim_type != PRIM_TYPE_TOKENS.skeleton {
            return prim;
        }
        let Some(prim_ds) = prim.data_source else {
            return prim;
        };
        let overlay = self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_skeleton
            .get(prim_path)
            .cloned();
        let Some(overlay) = overlay else {
            return HdSceneIndexPrim {
                prim_type: prim.prim_type,
                data_source: Some(prim_ds),
            };
        };

        HdSceneIndexPrim {
            prim_type: mesh_prim_type(),
            data_source: Some(HdOverlayContainerDataSource::new_2(overlay, prim_ds)),
        }
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(
        &self,
        _message_type: &Token,
        _args: Option<usd_hd::HdDataSourceBaseHandle>,
    ) {
    }

    fn get_display_name(&self) -> String {
        "SkeletonResolvingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for SkeletonResolvingSceneIndex {
    fn on_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let input_handle = match self.base.get_input_scene() {
            Some(i) => i.clone(),
            None => {
                self.base.forward_prims_added(self, entries);
                return;
            }
        };

        let mut has_skeletons = !self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_skeleton
            .is_empty();
        let mut skels_just_added = HashSet::new();
        let mut entry_indices = Vec::new();
        let mut new_entries = entries.to_vec();

        for (i, entry) in entries.iter().enumerate() {
            if has_skeletons {
                self.remove_resolved_skeleton(&entry.prim_path);
            }

            if entry.prim_type != PRIM_TYPE_TOKENS.skeleton {
                continue;
            }

            if self.add_resolved_skeleton(input_handle.clone(), &entry.prim_path) {
                new_entries[i].prim_type = mesh_prim_type();
                entry_indices.push(i);
                has_skeletons = true;
                skels_just_added.insert(entry.prim_path.clone());
            }
        }

        let mut new_dirtied_entries = Vec::new();
        let (skel_anim_dependencies, instancer_dependencies) = {
            let state = self.state.lock().expect("Lock poisoned");
            (
                state.skel_anim_path_to_skeleton_paths.clone(),
                state.instancer_path_to_skeleton_paths.clone(),
            )
        };
        if !skel_anim_dependencies.is_empty() || !instancer_dependencies.is_empty() {
            let mut skel_paths_needing_refresh = HashSet::new();
            for entry in entries {
                for prim_path in lookup(&skel_anim_dependencies, &entry.prim_path) {
                    if !skels_just_added.contains(prim_path) {
                        skel_paths_needing_refresh.insert(prim_path.clone());
                    }
                }
                for prim_path in lookup(&instancer_dependencies, &entry.prim_path) {
                    if !skels_just_added.contains(prim_path) {
                        skel_paths_needing_refresh.insert(prim_path.clone());
                    }
                }
            }

            for skel_path in skel_paths_needing_refresh {
                self.refresh_resolved_skeleton_data_source(input_handle.clone(), &skel_path);
                new_dirtied_entries.push(DirtiedPrimEntry::new(
                    skel_path,
                    HdDataSourceLocatorSet::universal(),
                ));
            }
        }

        if entries.len() >= 100 || !new_dirtied_entries.is_empty() {
            let first_path = entries
                .first()
                .map(|entry| entry.prim_path.as_ref().to_string())
                .unwrap_or_else(|| "<none>".to_string());
            log::info!(
                "[skeleton_resolving] on_prims_added in={} type_swaps={} extra_dirty={} sender={} first={}",
                entries.len(),
                entry_indices.len(),
                new_dirtied_entries.len(),
                sender.get_display_name(),
                first_path,
            );
        }

        if entry_indices.is_empty() {
            self.base.forward_prims_added(self, entries);
        } else {
            self.base.forward_prims_added(self, &new_entries);
        }
        if !new_dirtied_entries.is_empty() {
            if new_dirtied_entries.len() >= 1000 {
                let first = new_dirtied_entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[skeleton_resolving] branch=added.extra_dirtied out={} first={}",
                    new_dirtied_entries.len(),
                    first
                );
            }
            self.base.forward_prims_dirtied(self, &new_dirtied_entries);
        }
    }

    fn on_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_skeleton
            .is_empty()
        {
            self.base.forward_prims_removed(self, entries);
            return;
        }

        let input_handle = match self.base.get_input_scene() {
            Some(i) => i.clone(),
            None => {
                self.base.forward_prims_removed(self, entries);
                return;
            }
        };

        for entry in entries {
            let paths_to_remove: Vec<_> = {
                let state = self.state.lock().expect("Lock poisoned");
                state
                    .path_to_resolved_skeleton
                    .keys()
                    .filter(|path| path.has_prefix(&entry.prim_path))
                    .cloned()
                    .collect()
            };
            for path in paths_to_remove {
                self.remove_resolved_skeleton(&path);
            }
        }

        let mut new_dirtied_entries = Vec::new();
        let (skel_anim_dependencies, instancer_dependencies) = {
            let state = self.state.lock().expect("Lock poisoned");
            (
                state.skel_anim_path_to_skeleton_paths.clone(),
                state.instancer_path_to_skeleton_paths.clone(),
            )
        };
        if !skel_anim_dependencies.is_empty() || !instancer_dependencies.is_empty() {
            let mut skel_paths_needing_refresh = HashSet::new();
            for entry in entries {
                populate_from_dependencies(
                    &skel_anim_dependencies,
                    &entry.prim_path,
                    &mut skel_paths_needing_refresh,
                );
                populate_from_dependencies(
                    &instancer_dependencies,
                    &entry.prim_path,
                    &mut skel_paths_needing_refresh,
                );
            }

            for skel_path in skel_paths_needing_refresh {
                self.refresh_resolved_skeleton_data_source(input_handle.clone(), &skel_path);
                new_dirtied_entries.push(DirtiedPrimEntry::new(
                    skel_path,
                    HdDataSourceLocatorSet::universal(),
                ));
            }
        }

        if entries.len() >= 100 || !new_dirtied_entries.is_empty() {
            let first_path = entries
                .first()
                .map(|entry| entry.prim_path.as_ref().to_string())
                .unwrap_or_else(|| "<none>".to_string());
            log::info!(
                "[skeleton_resolving] on_prims_removed in={} extra_dirty={} sender={} first={}",
                entries.len(),
                new_dirtied_entries.len(),
                sender.get_display_name(),
                first_path,
            );
        }

        self.base.forward_prims_removed(self, entries);
        if !new_dirtied_entries.is_empty() {
            if new_dirtied_entries.len() >= 1000 {
                let first = new_dirtied_entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[skeleton_resolving] branch=removed.extra_dirtied out={} first={}",
                    new_dirtied_entries.len(),
                    first
                );
            }
            self.base.forward_prims_dirtied(self, &new_dirtied_entries);
        }
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_skeleton
            .is_empty()
        {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        let input_handle = match self.base.get_input_scene() {
            Some(i) => i.clone(),
            None => {
                self.base.forward_prims_dirtied(self, entries);
                return;
            }
        };

        let mut new_dirtied_entries = Vec::new();
        let sender_name = sender.get_display_name();
        let first_path = entries
            .first()
            .map(|entry| entry.prim_path.as_ref().to_string())
            .unwrap_or_else(|| "<none>".to_string());
        let dependent_locators =
            DataSourceResolvedSkeletonPrim::get_dependendend_on_data_source_locators();
        let mut instancer_locators = HdDataSourceLocatorSet::new();
        instancer_locators.insert(DataSourceXformResolver::get_instanced_by_locator());
        instancer_locators.insert(DataSourceXformResolver::get_xform_locator());
        instancer_locators.insert(DataSourceXformResolver::get_instance_xform_locator());
        instancer_locators.insert(DataSourceXformResolver::get_instance_animation_source_locator());
        let (skel_anim_dependencies, instancer_dependencies) = {
            let state = self.state.lock().expect("Lock poisoned");
            (
                state.skel_anim_path_to_skeleton_paths.clone(),
                state.instancer_path_to_skeleton_paths.clone(),
            )
        };

        for entry in entries {
            if entry.dirty_locators.intersects(&dependent_locators) {
                self.process_dirty_locators(
                    input_handle.clone(),
                    &entry.prim_path,
                    &PRIM_TYPE_TOKENS.skeleton,
                    &entry.dirty_locators,
                    &mut new_dirtied_entries,
                );
            }

            if entry
                .dirty_locators
                .intersects(&HdDataSourceLocatorSet::from_locator(
                    AnimationSchema::get_default_locator(),
                ))
            {
                let skel_paths: Vec<_> = lookup(&skel_anim_dependencies, &entry.prim_path)
                    .cloned()
                    .collect();
                for skel_path in skel_paths {
                    self.process_dirty_locators(
                        input_handle.clone(),
                        &skel_path,
                        &PRIM_TYPE_TOKENS.skel_animation,
                        &entry.dirty_locators,
                        &mut new_dirtied_entries,
                    );
                }
            }

            if entry.dirty_locators.intersects(&instancer_locators) {
                let skel_paths: Vec<_> = lookup(&instancer_dependencies, &entry.prim_path)
                    .cloned()
                    .collect();
                for skel_path in skel_paths {
                    self.process_dirty_locators(
                        input_handle.clone(),
                        &skel_path,
                        &Token::new("instancer"),
                        &entry.dirty_locators,
                        &mut new_dirtied_entries,
                    );
                }
            }
        }

        if entries.len() >= 100 || !new_dirtied_entries.is_empty() {
            log::info!(
                "[skeleton_resolving] on_prims_dirtied in={} extra_dirty={} sender={} first={}",
                entries.len(),
                new_dirtied_entries.len(),
                sender_name,
                first_path,
            );
        }

        if new_dirtied_entries.is_empty() {
            if entries.len() >= 1000 {
                eprintln!(
                    "[skeleton_resolving] branch=dirtied.passthrough out={} sender={} first={}",
                    entries.len(),
                    sender_name,
                    first_path,
                );
            }
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        let mut merged = entries.to_vec();
        merged.extend(new_dirtied_entries);
        if merged.len() >= 1000 {
            eprintln!(
                "[skeleton_resolving] branch=dirtied.merged in={} out={} sender={} first={}",
                entries.len(),
                merged.len(),
                sender_name,
                first_path,
            );
        }
        self.base.forward_prims_dirtied(self, &merged);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let input_handle = self.base.get_input_scene().cloned();

        for entry in entries {
            let paths_to_remove: Vec<_> = {
                let state = self.state.lock().expect("Lock poisoned");
                state
                    .path_to_resolved_skeleton
                    .keys()
                    .filter(|path| path.has_prefix(&entry.old_prim_path))
                    .cloned()
                    .collect()
            };
            for path in paths_to_remove {
                self.remove_resolved_skeleton(&path);
            }

            if let Some(input_handle) = &input_handle {
                let _ = self.add_resolved_skeleton(input_handle.clone(), &entry.new_prim_path);
            }
        }

        self.base.forward_prims_renamed(self, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage_scene_index::StageSceneIndex;
    use usd_core::{Stage, schema_registry::register_builtin_schemas};
    use usd_hd::scene_index::{arc_scene_index_to_handle, wire_filter_to_input};

    fn open_reference_stage(relative_under_usd_imaging_testenv: &str) -> Arc<Stage> {
        let fixture_path =
            openusd_test_path::pxr_usd_imaging_testenv(relative_under_usd_imaging_testenv);
        let p = fixture_path.to_string_lossy().replace('\\', "/");
        Stage::open(p.as_str(), usd_core::common::InitialLoadSet::LoadAll)
            .expect("open reference stage")
    }

    #[test]
    fn test_stage_after_wiring_populates_skeleton_resolution_state() {
        register_builtin_schemas();

        let stage = open_reference_stage("testUsdImagingGLSkeleton/skeleton.usda");
        let stage_scene_index = StageSceneIndex::new_with_input_args(None);
        let stage_handle = arc_scene_index_to_handle(stage_scene_index.clone());
        let resolving = SkeletonResolvingSceneIndex::new(stage_handle.clone());
        wire_filter_to_input(&resolving, &stage_handle);

        stage_scene_index.set_stage(stage);

        let skeleton_path =
            SdfPath::from_string("/SkelChar/Skeleton").expect("skeleton fixture path");
        let prim = resolving.read().get_prim(&skeleton_path);

        assert_eq!(prim.prim_type.as_str(), "mesh");
    }
}
