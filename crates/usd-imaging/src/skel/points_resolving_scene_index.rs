//! PointsResolvingSceneIndex - Adds ext computations to skinned points-based prims.
//!
//! Port of pxr/usdImaging/usdSkelImaging/pointsResolvingSceneIndex.h/cpp
//!
//! For mesh/basisCurves/points: overlays resolved skinning data, adds ext computation children.

use super::data_source_resolved_ext_computation_prim::data_source_resolved_ext_computation_prim;
use super::data_source_resolved_points_based_prim::DataSourceResolvedPointsBasedPrim;
use super::resolved_points_based_prim_container::ResolvedPointsBasedPrimContainer;
use super::resolved_skeleton_schema::ResolvedSkeletonSchema;
use super::tokens::{EXT_COMPUTATION_NAME_TOKENS, PRIM_TYPE_TOKENS};
use super::xform_resolver::DataSourceXformResolver;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocatorSet, HdOverlayContainerDataSource,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref, wire_filter_to_input,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Hd prim type tokens.
fn mesh_token() -> Token {
    Token::new("mesh")
}
fn basis_curves_token() -> Token {
    Token::new("basisCurves")
}
fn points_token() -> Token {
    Token::new("points")
}
fn ext_computation_token() -> Token {
    Token::new("extComputation")
}

fn is_point_based_prim(prim_type: &Token) -> bool {
    *prim_type == mesh_token() || *prim_type == basis_curves_token() || *prim_type == points_token()
}

/// Ext computation name tokens as slice for iteration.
fn ext_computation_names() -> [Token; 4] {
    [
        EXT_COMPUTATION_NAME_TOKENS
            .points_aggregator_computation
            .clone(),
        EXT_COMPUTATION_NAME_TOKENS.points_computation.clone(),
        EXT_COMPUTATION_NAME_TOKENS
            .normals_aggregator_computation
            .clone(),
        EXT_COMPUTATION_NAME_TOKENS.normals_computation.clone(),
    ]
}

/// Scene index that adds skinning ext computations to points-based prims.
pub struct PointsResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    state: Mutex<PointsResolvingState>,
}

struct PointsResolvingState {
    /// Map prim path -> resolved data source overlay.
    path_to_resolved_prim: HashMap<SdfPath, Arc<DataSourceResolvedPointsBasedPrim>>,
    /// Reverse dependencies from skeleton path to points-based prim paths.
    skel_path_to_prim_paths: HashMap<SdfPath, HashSet<SdfPath>>,
    /// Reverse dependencies from blend shape target path to prim paths.
    blend_shape_path_to_prim_paths: HashMap<SdfPath, HashSet<SdfPath>>,
    /// Reverse dependencies from instancer path to prim paths.
    instancer_path_to_prim_paths: HashMap<SdfPath, HashSet<SdfPath>>,
}

impl PointsResolvingState {
    fn new() -> Self {
        Self {
            path_to_resolved_prim: HashMap::new(),
            skel_path_to_prim_paths: HashMap::new(),
            blend_shape_path_to_prim_paths: HashMap::new(),
            instancer_path_to_prim_paths: HashMap::new(),
        }
    }
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
    prim_paths: &mut HashMap<SdfPath, bool>,
) {
    for (dependency_path, affected_paths) in dependencies {
        if dependency_path.has_prefix(prefix) {
            for prim_path in affected_paths {
                prim_paths.entry(prim_path.clone()).or_insert(false);
            }
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

fn add_dependencies_for_resolved_prim_state(
    state: &mut PointsResolvingState,
    prim_path: &SdfPath,
    resolved: &Arc<DataSourceResolvedPointsBasedPrim>,
) {
    let skeleton_path = resolved.get_skeleton_path();
    if !skeleton_path.is_empty() {
        state
            .skel_path_to_prim_paths
            .entry(skeleton_path.clone())
            .or_default()
            .insert(prim_path.clone());
    }

    for blend_shape_path in resolved.get_blend_shape_target_paths() {
        if !blend_shape_path.is_empty() {
            state
                .blend_shape_path_to_prim_paths
                .entry(blend_shape_path.clone())
                .or_default()
                .insert(prim_path.clone());
        }
    }

    if resolved.has_ext_computations() {
        for instancer_path in resolved.get_instancer_paths() {
            if !instancer_path.is_empty() {
                state
                    .instancer_path_to_prim_paths
                    .entry(instancer_path.clone())
                    .or_default()
                    .insert(prim_path.clone());
            }
        }
    }
}

fn remove_dependencies_for_resolved_prim_state(
    state: &mut PointsResolvingState,
    prim_path: &SdfPath,
    resolved: &Arc<DataSourceResolvedPointsBasedPrim>,
) {
    let skeleton_path = resolved.get_skeleton_path();
    if !skeleton_path.is_empty() {
        remove_dependency(&mut state.skel_path_to_prim_paths, skeleton_path, prim_path);
    }

    for blend_shape_path in resolved.get_blend_shape_target_paths() {
        if !blend_shape_path.is_empty() {
            remove_dependency(
                &mut state.blend_shape_path_to_prim_paths,
                blend_shape_path,
                prim_path,
            );
        }
    }

    for instancer_path in resolved.get_instancer_paths() {
        if !instancer_path.is_empty() {
            remove_dependency(
                &mut state.instancer_path_to_prim_paths,
                instancer_path,
                prim_path,
            );
        }
    }
}

impl std::fmt::Debug for PointsResolvingSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointsResolvingSceneIndex")
            .field(
                "resolved_count",
                &self
                    .state
                    .lock()
                    .expect("Lock poisoned")
                    .path_to_resolved_prim
                    .len(),
            )
            .finish()
    }
}

impl PointsResolvingSceneIndex {
    /// Create a new points resolving scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            state: Mutex::new(PointsResolvingState::new()),
        }));
        wire_filter_to_input(&this, &input_scene);

        // Initial population
        let input_handle = this.read().base.get_input_scene().cloned();
        if let Some(input_handle) = input_handle {
            let input_locked = input_handle.read();
            let guard = this.read();
            for path in Self::collect_prim_paths(&*input_locked, &SdfPath::absolute_root()) {
                let _ = guard.add_resolved_prim(input_handle.clone(), &path);
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

    fn add_resolved_prim(&self, input_handle: HdSceneIndexHandle, prim_path: &SdfPath) -> bool {
        let prim = si_ref(&input_handle).get_prim(prim_path);
        if !is_point_based_prim(&prim.prim_type) {
            return false;
        }
        let Some(ref prim_source) = prim.data_source else {
            return false;
        };

        if let Some(resolved) =
            DataSourceResolvedPointsBasedPrim::new_from_scene(input_handle, prim_path, prim_source)
        {
            let mut state = self.state.lock().expect("Lock poisoned");
            if let Some(previous) = state.path_to_resolved_prim.remove(prim_path) {
                remove_dependencies_for_resolved_prim_state(&mut state, prim_path, &previous);
            }
            add_dependencies_for_resolved_prim_state(&mut state, prim_path, &resolved);
            state
                .path_to_resolved_prim
                .insert(prim_path.clone(), resolved);
            true
        } else {
            false
        }
    }

    fn refresh_resolved_prim_data_source(
        &self,
        input_handle: HdSceneIndexHandle,
        prim_path: &SdfPath,
    ) -> bool {
        let _ = self.remove_resolved_prim(prim_path);
        self.add_resolved_prim(input_handle, prim_path)
    }

    fn remove_resolved_prim(
        &self,
        prim_path: &SdfPath,
    ) -> Option<Arc<DataSourceResolvedPointsBasedPrim>> {
        let mut state = self.state.lock().expect("Lock poisoned");
        let removed = state.path_to_resolved_prim.remove(prim_path);
        if let Some(ref resolved) = removed {
            remove_dependencies_for_resolved_prim_state(&mut state, prim_path, resolved);
        }
        removed
    }

    fn process_dirty_locators(
        &self,
        input_handle: HdSceneIndexHandle,
        prim_path: &SdfPath,
        dirtied_prim_type: &Token,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: &mut Vec<DirtiedPrimEntry>,
    ) -> bool {
        let resolved = {
            self.state
                .lock()
                .expect("Lock poisoned")
                .path_to_resolved_prim
                .get(prim_path)
                .cloned()
        };

        let Some(resolved) = resolved else {
            return false;
        };

        if !resolved.process_dirty_locators(dirtied_prim_type, dirty_locators, Some(entries)) {
            return false;
        }

        self.refresh_resolved_prim_data_source(input_handle, prim_path)
    }

    fn process_prims_needing_refresh_and_send_notices(
        &self,
        input_handle: HdSceneIndexHandle,
        prims_needing_refresh_to_has_added_entry: &HashMap<SdfPath, bool>,
        mut added_entries: Option<&mut Vec<AddedPrimEntry>>,
        mut removed_entries: Option<&mut Vec<RemovedPrimEntry>>,
        mut dirtied_entries: Option<&mut Vec<DirtiedPrimEntry>>,
    ) {
        for (prim_path, has_added_entry) in prims_needing_refresh_to_has_added_entry {
            let had_ext_computations = self
                .state
                .lock()
                .expect("Lock poisoned")
                .path_to_resolved_prim
                .get(prim_path)
                .map(|resolved| resolved.has_ext_computations())
                .unwrap_or(false);

            let removed = self.remove_resolved_prim(prim_path).is_some();
            let has_ext_computations = {
                let added = self.add_resolved_prim(input_handle.clone(), prim_path);
                if added {
                    self.state
                        .lock()
                        .expect("Lock poisoned")
                        .path_to_resolved_prim
                        .get(prim_path)
                        .map(|resolved| resolved.has_ext_computations())
                        .unwrap_or(false)
                } else {
                    false
                }
            };
            let added = has_ext_computations
                || self
                    .state
                    .lock()
                    .expect("Lock poisoned")
                    .path_to_resolved_prim
                    .contains_key(prim_path);

            if let Some(dirtied_entries) = dirtied_entries.as_mut() {
                if !*has_added_entry && (removed || added) {
                    dirtied_entries.push(DirtiedPrimEntry::new(
                        prim_path.clone(),
                        HdDataSourceLocatorSet::universal(),
                    ));
                    if has_ext_computations {
                        for name in ext_computation_names() {
                            if let Some(child_path) = prim_path.append_child(name.as_str()) {
                                dirtied_entries.push(DirtiedPrimEntry::new(
                                    child_path,
                                    HdDataSourceLocatorSet::universal(),
                                ));
                            }
                        }
                    }
                }
            }

            if let Some(removed_entries) = removed_entries.as_mut() {
                if had_ext_computations && !has_ext_computations {
                    for name in ext_computation_names() {
                        if let Some(child_path) = prim_path.append_child(name.as_str()) {
                            removed_entries.push(RemovedPrimEntry::new(child_path));
                        }
                    }
                }
            }

            if let Some(added_entries) = added_entries.as_mut() {
                if has_ext_computations && !had_ext_computations {
                    for name in ext_computation_names() {
                        if let Some(child_path) = prim_path.append_child(name.as_str()) {
                            added_entries
                                .push(AddedPrimEntry::new(child_path, ext_computation_token()));
                        }
                    }
                }
            }
        }

        let added_count = added_entries
            .as_ref()
            .map(|entries| entries.len())
            .unwrap_or(0);
        let removed_count = removed_entries
            .as_ref()
            .map(|entries| entries.len())
            .unwrap_or(0);
        let dirtied_count = dirtied_entries
            .as_ref()
            .map(|entries| entries.len())
            .unwrap_or(0);
        if prims_needing_refresh_to_has_added_entry.len() >= 100
            || added_count != 0
            || removed_count != 0
            || dirtied_count != 0
        {
            let with_added_entry = prims_needing_refresh_to_has_added_entry
                .values()
                .filter(|has_added_entry| **has_added_entry)
                .count();
            let first = prims_needing_refresh_to_has_added_entry
                .keys()
                .next()
                .map(ToString::to_string)
                .unwrap_or_default();
            log::info!(
                "points_resolving refresh_summary prims={} has_added_entry={} added={} removed={} dirtied={} first={}",
                prims_needing_refresh_to_has_added_entry.len(),
                with_added_entry,
                added_count,
                removed_count,
                dirtied_count,
                first
            );
            eprintln!(
                "[points_resolving] refresh_summary prims={} has_added_entry={} added={} removed={} dirtied={} first={}",
                prims_needing_refresh_to_has_added_entry.len(),
                with_added_entry,
                added_count,
                removed_count,
                dirtied_count,
                first
            );
        }
    }
}

impl HdSceneIndexBase for PointsResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };
        let input_locked = input.read();

        let mut prim = input_locked.get_prim(prim_path);

        if is_point_based_prim(&prim.prim_type) {
            if let Some(resolved) = self
                .state
                .lock()
                .expect("Lock poisoned")
                .path_to_resolved_prim
                .get(prim_path)
                .cloned()
            {
                if let Some(ref prim_ds) = prim.data_source {
                    let overlay: HdContainerDataSourceHandle =
                        ResolvedPointsBasedPrimContainer::build_overlay(resolved);
                    prim.data_source = Some(HdOverlayContainerDataSource::new_2(
                        overlay,
                        prim_ds.clone(),
                    ));
                }
            }
        }

        // Ext computation: prim path like /Mesh/skinningPointsComputation
        let computation_name = prim_path.get_name_token();
        let parent_path = prim_path.get_parent_path();
        if let Some(resolved) = self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_prim
            .get(&parent_path)
            .cloned()
        {
            if resolved.has_ext_computations() {
                for name in ext_computation_names() {
                    if computation_name == name {
                        if let Some(ds) =
                            data_source_resolved_ext_computation_prim(resolved.clone(), &name)
                        {
                            return HdSceneIndexPrim {
                                prim_type: ext_computation_token(),
                                data_source: Some(ds),
                            };
                        }
                    }
                }
            }
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let mut result = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_child_prim_paths(prim_path)
        } else {
            Vec::new()
        };

        // Add ext computation children for resolved skinned prims
        if let Some(resolved) = self
            .state
            .lock()
            .expect("Lock poisoned")
            .path_to_resolved_prim
            .get(prim_path)
            .cloned()
        {
            if resolved.has_ext_computations() {
                for name in ext_computation_names() {
                    if let Some(child_path) = prim_path.append_child(name.as_str()) {
                        result.push(child_path);
                    }
                }
            }
        }

        result
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
        "PointsResolvingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for PointsResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let input_handle = match self.base.get_input_scene() {
            Some(i) => i.clone(),
            None => {
                self.base.forward_prims_added(self, entries);
                return;
            }
        };

        let has_dependencies = {
            let state = self.state.lock().expect("Lock poisoned");
            !state.skel_path_to_prim_paths.is_empty()
                || !state.blend_shape_path_to_prim_paths.is_empty()
                || !state.instancer_path_to_prim_paths.is_empty()
        };

        let mut prims_needing_refresh_to_has_added_entry = HashMap::new();
        for entry in entries {
            if is_point_based_prim(&entry.prim_type)
                || self.remove_resolved_prim(&entry.prim_path).is_some()
            {
                prims_needing_refresh_to_has_added_entry.insert(entry.prim_path.clone(), true);
            }
        }

        if has_dependencies {
            let (skel_dependencies, blend_dependencies, instancer_dependencies) = {
                let state = self.state.lock().expect("Lock poisoned");
                (
                    state.skel_path_to_prim_paths.clone(),
                    state.blend_shape_path_to_prim_paths.clone(),
                    state.instancer_path_to_prim_paths.clone(),
                )
            };

            for entry in entries {
                for prim_path in lookup(&skel_dependencies, &entry.prim_path) {
                    prims_needing_refresh_to_has_added_entry
                        .entry(prim_path.clone())
                        .or_insert(false);
                }
                for prim_path in lookup(&blend_dependencies, &entry.prim_path) {
                    prims_needing_refresh_to_has_added_entry
                        .entry(prim_path.clone())
                        .or_insert(false);
                }
                for prim_path in lookup(&instancer_dependencies, &entry.prim_path) {
                    prims_needing_refresh_to_has_added_entry
                        .entry(prim_path.clone())
                        .or_insert(false);
                }
            }
        }

        if prims_needing_refresh_to_has_added_entry.is_empty() {
            self.base.forward_prims_added(self, entries);
            return;
        }

        let mut new_added_entries = Vec::new();
        let mut new_removed_entries = Vec::new();
        let mut new_dirtied_entries = Vec::new();
        self.process_prims_needing_refresh_and_send_notices(
            input_handle,
            &prims_needing_refresh_to_has_added_entry,
            Some(&mut new_added_entries),
            Some(&mut new_removed_entries),
            Some(&mut new_dirtied_entries),
        );

        if entries.len() >= 100
            || !new_added_entries.is_empty()
            || !new_removed_entries.is_empty()
            || !new_dirtied_entries.is_empty()
        {
            let first = entries
                .first()
                .map(|entry| entry.prim_path.to_string())
                .unwrap_or_default();
            log::info!(
                "points_resolving on_prims_added in={} refresh={} synth_added={} synth_removed={} synth_dirtied={} first={}",
                entries.len(),
                prims_needing_refresh_to_has_added_entry.len(),
                new_added_entries.len(),
                new_removed_entries.len(),
                new_dirtied_entries.len(),
                first
            );
            eprintln!(
                "[points_resolving] on_prims_added in={} refresh={} synth_added={} synth_removed={} synth_dirtied={} first={}",
                entries.len(),
                prims_needing_refresh_to_has_added_entry.len(),
                new_added_entries.len(),
                new_removed_entries.len(),
                new_dirtied_entries.len(),
                first
            );
        }

        self.base.forward_prims_removed(self, &new_removed_entries);
        if new_added_entries.is_empty() {
            self.base.forward_prims_added(self, entries);
        } else {
            let mut merged = entries.to_vec();
            merged.extend(new_added_entries);
            self.base.forward_prims_added(self, &merged);
        }
        if !new_dirtied_entries.is_empty() {
            if new_dirtied_entries.len() >= 1000 {
                let first = new_dirtied_entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[points_resolving] branch=added.extra_dirtied out={} first={}",
                    new_dirtied_entries.len(),
                    first
                );
            }
            self.base.forward_prims_dirtied(self, &new_dirtied_entries);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        for entry in entries {
            let paths_to_remove: Vec<_> = {
                let state = self.state.lock().expect("Lock poisoned");
                state
                    .path_to_resolved_prim
                    .keys()
                    .filter(|path| path.has_prefix(&entry.prim_path))
                    .cloned()
                    .collect()
            };
            for path in paths_to_remove {
                self.remove_resolved_prim(&path);
            }
        }

        let (has_skel_dependencies, has_blend_dependencies, has_instancer_dependencies) = {
            let state = self.state.lock().expect("Lock poisoned");
            (
                !state.skel_path_to_prim_paths.is_empty(),
                !state.blend_shape_path_to_prim_paths.is_empty(),
                !state.instancer_path_to_prim_paths.is_empty(),
            )
        };

        if !has_skel_dependencies && !has_blend_dependencies && !has_instancer_dependencies {
            self.base.forward_prims_removed(self, entries);
            return;
        }

        let mut prims_needing_refresh_to_has_added_entry = HashMap::new();
        let (skel_dependencies, blend_dependencies, instancer_dependencies) = {
            let state = self.state.lock().expect("Lock poisoned");
            (
                state.skel_path_to_prim_paths.clone(),
                state.blend_shape_path_to_prim_paths.clone(),
                state.instancer_path_to_prim_paths.clone(),
            )
        };

        for entry in entries {
            if has_skel_dependencies {
                populate_from_dependencies(
                    &skel_dependencies,
                    &entry.prim_path,
                    &mut prims_needing_refresh_to_has_added_entry,
                );
            }
            if has_blend_dependencies {
                populate_from_dependencies(
                    &blend_dependencies,
                    &entry.prim_path,
                    &mut prims_needing_refresh_to_has_added_entry,
                );
            }
            if has_instancer_dependencies {
                populate_from_dependencies(
                    &instancer_dependencies,
                    &entry.prim_path,
                    &mut prims_needing_refresh_to_has_added_entry,
                );
            }
        }

        if prims_needing_refresh_to_has_added_entry.is_empty() {
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

        let mut new_added_entries = Vec::new();
        let mut new_removed_entries = Vec::new();
        let mut new_dirtied_entries = Vec::new();
        self.process_prims_needing_refresh_and_send_notices(
            input_handle,
            &prims_needing_refresh_to_has_added_entry,
            Some(&mut new_added_entries),
            Some(&mut new_removed_entries),
            Some(&mut new_dirtied_entries),
        );

        if entries.len() >= 100
            || !new_added_entries.is_empty()
            || !new_removed_entries.is_empty()
            || !new_dirtied_entries.is_empty()
        {
            let first = entries
                .first()
                .map(|entry| entry.prim_path.to_string())
                .unwrap_or_default();
            log::info!(
                "points_resolving on_prims_removed in={} refresh={} synth_added={} synth_removed={} synth_dirtied={} first={}",
                entries.len(),
                prims_needing_refresh_to_has_added_entry.len(),
                new_added_entries.len(),
                new_removed_entries.len(),
                new_dirtied_entries.len(),
                first
            );
            eprintln!(
                "[points_resolving] on_prims_removed in={} refresh={} synth_added={} synth_removed={} synth_dirtied={} first={}",
                entries.len(),
                prims_needing_refresh_to_has_added_entry.len(),
                new_added_entries.len(),
                new_removed_entries.len(),
                new_dirtied_entries.len(),
                first
            );
        }

        let mut merged_removed = entries.to_vec();
        merged_removed.extend(new_removed_entries);
        self.base.forward_prims_removed(self, &merged_removed);
        if !new_added_entries.is_empty() {
            self.base.forward_prims_added(self, &new_added_entries);
        }
        if !new_dirtied_entries.is_empty() {
            if new_dirtied_entries.len() >= 1000 {
                let first = new_dirtied_entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[points_resolving] branch=removed.extra_dirtied out={} first={}",
                    new_dirtied_entries.len(),
                    first
                );
            }
            self.base.forward_prims_dirtied(self, &new_dirtied_entries);
        }
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let has_resolved_prims = {
            let state = self.state.lock().expect("Lock poisoned");
            !state.path_to_resolved_prim.is_empty()
        };
        if !has_resolved_prims {
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
        let dependent_locators =
            DataSourceResolvedPointsBasedPrim::get_dependendend_on_data_source_locators();
        let mut instancer_locators = HdDataSourceLocatorSet::new();
        instancer_locators.insert(DataSourceXformResolver::get_instanced_by_locator());
        instancer_locators.insert(DataSourceXformResolver::get_xform_locator());
        instancer_locators.insert(DataSourceXformResolver::get_instance_xform_locator());

        let (skel_dependencies, blend_dependencies, instancer_dependencies) = {
            let state = self.state.lock().expect("Lock poisoned");
            (
                state.skel_path_to_prim_paths.clone(),
                state.blend_shape_path_to_prim_paths.clone(),
                state.instancer_path_to_prim_paths.clone(),
            )
        };

        let mut prims_needing_refresh_to_has_added_entry = HashMap::new();

        for entry in entries {
            if !has_resolved_prims
                && !entry
                    .dirty_locators
                    .intersects(&HdDataSourceLocatorSet::from_locator(
                        super::binding_schema::BindingSchema::get_skeleton_locator(),
                    ))
            {
                continue;
            }

            if entry.dirty_locators.intersects(&dependent_locators)
                && self.process_dirty_locators(
                    input_handle.clone(),
                    &entry.prim_path,
                    &Token::new(""),
                    &entry.dirty_locators,
                    &mut new_dirtied_entries,
                )
            {
                prims_needing_refresh_to_has_added_entry.insert(entry.prim_path.clone(), false);
            }

            if entry
                .dirty_locators
                .intersects(&HdDataSourceLocatorSet::from_locator(
                    ResolvedSkeletonSchema::get_default_locator(),
                ))
            {
                let prim_paths: Vec<_> = lookup(&skel_dependencies, &entry.prim_path)
                    .cloned()
                    .collect();
                for prim_path in prim_paths {
                    if self.process_dirty_locators(
                        input_handle.clone(),
                        &prim_path,
                        &PRIM_TYPE_TOKENS.skeleton,
                        &entry.dirty_locators,
                        &mut new_dirtied_entries,
                    ) {
                        prims_needing_refresh_to_has_added_entry.insert(prim_path, false);
                    }
                }
            }

            if entry
                .dirty_locators
                .intersects(&HdDataSourceLocatorSet::from_locator(
                    super::blend_shape_schema::BlendShapeSchema::get_default_locator(),
                ))
            {
                let prim_paths: Vec<_> = lookup(&blend_dependencies, &entry.prim_path)
                    .cloned()
                    .collect();
                for prim_path in prim_paths {
                    if self.process_dirty_locators(
                        input_handle.clone(),
                        &prim_path,
                        &PRIM_TYPE_TOKENS.skel_blend_shape,
                        &entry.dirty_locators,
                        &mut new_dirtied_entries,
                    ) {
                        prims_needing_refresh_to_has_added_entry.insert(prim_path, false);
                    }
                }
            }

            if entry.dirty_locators.intersects(&instancer_locators) {
                let prim_paths: Vec<_> = lookup(&instancer_dependencies, &entry.prim_path)
                    .cloned()
                    .collect();
                for prim_path in prim_paths {
                    if self.process_dirty_locators(
                        input_handle.clone(),
                        &prim_path,
                        &Token::new("instancer"),
                        &entry.dirty_locators,
                        &mut new_dirtied_entries,
                    ) {
                        prims_needing_refresh_to_has_added_entry.insert(prim_path, false);
                    }
                }
            }
        }

        if !prims_needing_refresh_to_has_added_entry.is_empty() {
            let mut new_added_entries = Vec::new();
            let mut new_removed_entries = Vec::new();
            self.process_prims_needing_refresh_and_send_notices(
                input_handle,
                &prims_needing_refresh_to_has_added_entry,
                Some(&mut new_added_entries),
                Some(&mut new_removed_entries),
                Some(&mut new_dirtied_entries),
            );
            if entries.len() >= 100
                || !new_added_entries.is_empty()
                || !new_removed_entries.is_empty()
                || !new_dirtied_entries.is_empty()
            {
                let first = entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                log::info!(
                    "points_resolving on_prims_dirtied in={} sender={} refresh={} synth_added={} synth_removed={} total_synth_dirtied={} first={}",
                    entries.len(),
                    sender.get_display_name(),
                    prims_needing_refresh_to_has_added_entry.len(),
                    new_added_entries.len(),
                    new_removed_entries.len(),
                    new_dirtied_entries.len(),
                    first
                );
                eprintln!(
                    "[points_resolving] on_prims_dirtied in={} sender={} refresh={} synth_added={} synth_removed={} total_synth_dirtied={} first={}",
                    entries.len(),
                    sender.get_display_name(),
                    prims_needing_refresh_to_has_added_entry.len(),
                    new_added_entries.len(),
                    new_removed_entries.len(),
                    new_dirtied_entries.len(),
                    first
                );
            }
            if !new_removed_entries.is_empty() {
                self.base.forward_prims_removed(self, &new_removed_entries);
            }
            if !new_added_entries.is_empty() {
                self.base.forward_prims_added(self, &new_added_entries);
            }
        }

        if new_dirtied_entries.is_empty() {
            if entries.len() >= 100 {
                let first = entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                log::info!(
                    "points_resolving forward_passthrough_dirtied in={} sender={} first={}",
                    entries.len(),
                    sender.get_display_name(),
                    first
                );
                eprintln!(
                    "[points_resolving] forward_passthrough_dirtied in={} sender={} first={}",
                    entries.len(),
                    sender.get_display_name(),
                    first
                );
            }
            if entries.len() >= 1000 {
                let first = entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[points_resolving] branch=dirtied.passthrough out={} sender={} first={}",
                    entries.len(),
                    sender.get_display_name(),
                    first
                );
            }
            self.base.forward_prims_dirtied(self, entries);
        } else {
            let mut merged = entries.to_vec();
            merged.extend(new_dirtied_entries);
            let first = merged
                .first()
                .map(|entry| entry.prim_path.to_string())
                .unwrap_or_default();
            log::info!(
                "points_resolving forward_merged_dirtied in={} merged={} sender={} first={}",
                entries.len(),
                merged.len(),
                sender.get_display_name(),
                first
            );
            eprintln!(
                "[points_resolving] forward_merged_dirtied in={} merged={} sender={} first={}",
                entries.len(),
                merged.len(),
                sender.get_display_name(),
                first
            );
            if merged.len() >= 1000 {
                eprintln!(
                    "[points_resolving] branch=dirtied.merged in={} out={} sender={} first={}",
                    entries.len(),
                    merged.len(),
                    sender.get_display_name(),
                    first
                );
            }
            self.base.forward_prims_dirtied(self, &merged);
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let input_handle = self.base.get_input_scene().cloned();
        for entry in entries {
            let paths_to_remove: Vec<_> = {
                let state = self.state.lock().expect("Lock poisoned");
                state
                    .path_to_resolved_prim
                    .keys()
                    .filter(|path| path.has_prefix(&entry.old_prim_path))
                    .cloned()
                    .collect()
            };
            for path in paths_to_remove {
                self.remove_resolved_prim(&path);
            }

            if let Some(input_handle) = &input_handle {
                let _ = self.add_resolved_prim(input_handle.clone(), &entry.new_prim_path);
            }
        }
        self.base.forward_prims_renamed(self, entries);
    }
}
