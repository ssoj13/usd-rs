
//! Light linking scene index.
//!
//! Processes light linking relationships between lights and geometry.
//! Resolves collection-based light/shadow linking to category IDs and
//! overlays categories on geometry prims.
//!
//! Port of pxr/imaging/hdsi/lightLinkingSceneIndex.

use crate::utils::HdCollectionExpressionEvaluator;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedSmallVectorDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
};
use usd_hd::schema::{
    HdCategoriesSchema, HdCollectionsSchema, HdDependenciesSchema, HdDependencySchemaBuilder,
    HdInstanceCategoriesSchema, HdInstancedBySchema, HdInstancerTopologySchema, HdLightSchema,
    HdLocatorDataSourceHandle, HdPathDataSourceHandle,
};
use usd_hd::tokens;
use usd_sdf::Path as SdfPath;
use usd_sdf::PathExpression;
use usd_tf::Token as TfToken;

/// Tokens for light linking.
mod light_tokens {
    use once_cell::sync::Lazy;
    use usd_tf::Token;

    pub static LIGHT_LINK: Lazy<Token> = Lazy::new(|| Token::new("lightLink"));
    pub static SHADOW_LINK: Lazy<Token> = Lazy::new(|| Token::new("shadowLink"));
    pub static FILTER_LINK: Lazy<Token> = Lazy::new(|| Token::new("filterLink"));
    pub static LIGHT_FILTER_LINK: Lazy<Token> = Lazy::new(|| Token::new("lightFilterLink"));
}

type CollectionId = (SdfPath, TfToken);

static EMPTY_PRIM_CONTAINER: once_cell::sync::Lazy<HdContainerDataSourceHandle> =
    once_cell::sync::Lazy::new(|| HdRetainedContainerDataSource::new_empty());

fn get_all_linking_collection_names() -> &'static [TfToken] {
    static NAMES: once_cell::sync::Lazy<[TfToken; 3]> = once_cell::sync::Lazy::new(|| {
        [
            light_tokens::LIGHT_LINK.clone(),
            light_tokens::SHADOW_LINK.clone(),
            light_tokens::FILTER_LINK.clone(),
        ]
    });
    NAMES.as_slice()
}

fn get_light_linking_schema_tokens() -> &'static [TfToken] {
    static TOKENS: once_cell::sync::Lazy<[TfToken; 3]> = once_cell::sync::Lazy::new(|| {
        [
            light_tokens::LIGHT_LINK.clone(),
            light_tokens::SHADOW_LINK.clone(),
            light_tokens::LIGHT_FILTER_LINK.clone(),
        ]
    });
    TOKENS.as_slice()
}

fn is_instanced(prim_container: &HdContainerDataSourceHandle) -> bool {
    let schema = HdInstancedBySchema::get_from_parent(prim_container);
    schema
        .get_paths()
        .map(|ds| !ds.get_typed_value(0.0).is_empty())
        .unwrap_or(false)
}

fn build_categories_data_source(
    cache: &LightLinkingCache,
    prim_path: &SdfPath,
) -> Option<HdContainerDataSourceHandle> {
    let categories = cache.compute_categories_for_prim_path(prim_path);
    if categories.is_empty() {
        return None;
    }
    Some(HdCategoriesSchema::build_retained(&categories, &[]))
}

fn build_instance_categories_data_source(
    cache: &LightLinkingCache,
    _instancer_prim_path: &SdfPath,
    instancer_prim_ds: &HdContainerDataSourceHandle,
) -> Option<HdContainerDataSourceHandle> {
    let topology = HdInstancerTopologySchema::get_from_parent(instancer_prim_ds);
    let instance_paths_ds = topology.get_instance_locations()?;
    let paths: Vec<SdfPath> = instance_paths_ds.get_typed_value(0.0);
    if paths.is_empty() {
        return None;
    }
    let mut data_sources: Vec<HdDataSourceBaseHandle> = Vec::with_capacity(paths.len());
    for path in &paths {
        if let Some(ds) = build_categories_data_source(cache, path) {
            data_sources.push(ds as HdDataSourceBaseHandle);
        } else {
            data_sources.push(HdRetainedContainerDataSource::new_empty() as HdDataSourceBaseHandle);
        }
    }
    let vec_ds = HdRetainedSmallVectorDataSource::new(&data_sources);
    Some(HdInstanceCategoriesSchema::build_retained(Some(
        vec_ds as std::sync::Arc<dyn usd_hd::HdVectorDataSource>,
    )))
}

fn build_dependencies_data_source(
    instancer_prim_container: &HdContainerDataSourceHandle,
) -> Option<HdContainerDataSourceHandle> {
    let topology = HdInstancerTopologySchema::get_from_parent(instancer_prim_container);
    let instance_paths_ds = topology.get_instance_locations()?;
    let paths: Vec<SdfPath> = instance_paths_ds.get_typed_value(0.0);
    if paths.is_empty() {
        return None;
    }
    let categories_loc =
        HdRetainedTypedSampledDataSource::new(HdCategoriesSchema::get_default_locator());
    let instance_categories_loc =
        HdRetainedTypedSampledDataSource::new(HdInstanceCategoriesSchema::get_default_locator());
    let mut names = Vec::new();
    let mut sources: Vec<std::sync::Arc<dyn usd_hd::data_source::HdDataSourceBase>> = Vec::new();
    for (idx, instance_path) in paths.into_iter().enumerate() {
        names.push(TfToken::new(&format!("dep_{}", idx)));
        let dep =
            HdDependencySchemaBuilder::default()
                .set_depended_on_prim_path(
                    HdRetainedTypedSampledDataSource::new(instance_path) as HdPathDataSourceHandle
                )
                .set_depended_on_data_source_locator(
                    categories_loc.clone() as HdLocatorDataSourceHandle
                )
                .set_affected_data_source_locator(
                    instance_categories_loc.clone() as HdLocatorDataSourceHandle
                )
                .build();
        sources.push(dep as std::sync::Arc<dyn usd_hd::data_source::HdDataSourceBase>);
    }
    Some(HdDependenciesSchema::build_retained(&names, &sources))
}

/// Cache for light linking collections.
struct LightLinkingCache {
    scene_index: HdSceneIndexHandle,
    expr_to_category_id_and_eval: HashMap<String, (TfToken, HdCollectionExpressionEvaluator)>,
    category_id_to_expr: HashMap<TfToken, PathExpression>,
    collection_id_to_category_id: HashMap<CollectionId, TfToken>,
    category_id_to_collection_ids: HashMap<TfToken, HashSet<CollectionId>>,
    dirty_state: Vec<(PathExpression, Option<CollectionId>)>,
    group_idx: usize,
}

impl LightLinkingCache {
    fn new(scene_index: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            scene_index,
            expr_to_category_id_and_eval: HashMap::new(),
            category_id_to_expr: HashMap::new(),
            collection_id_to_category_id: HashMap::new(),
            category_id_to_collection_ids: HashMap::new(),
            dirty_state: Vec::new(),
            group_idx: 0,
        }))
    }

    fn process_collection(
        me: &Arc<RwLock<Self>>,
        prim_path: &SdfPath,
        collection_name: &TfToken,
        expr: PathExpression,
    ) {
        let collection_id = (prim_path.clone(), collection_name.clone());
        let mut guard = me.write();
        let cache = &mut *guard;

        let category_id_opt = cache.collection_id_to_category_id.get(&collection_id);
        let _collection_exists = category_id_opt.is_some();

        if let Some(category_id) = category_id_opt {
            if category_id.is_empty() {
                return;
            }
            if let Some(old_expr) = cache.category_id_to_expr.get(category_id) {
                if *old_expr == expr {
                    return;
                }
                cache
                    .dirty_state
                    .push((old_expr.clone(), Some(collection_id.clone())));
            }
            Self::remove_collection_inner(cache, &collection_id, true);
        }

        if PathExpression::is_trivial(&expr) {
            return;
        }

        let expr_text = expr.get_text();
        let entry = cache
            .expr_to_category_id_and_eval
            .entry(expr_text.clone())
            .or_insert_with(|| {
                let new_id = format!("group_{}", cache.group_idx);
                cache.group_idx += 1;
                let token = TfToken::new(&new_id);
                let eval = HdCollectionExpressionEvaluator::new(
                    cache.scene_index.clone(),
                    &PathExpression::parse(&expr_text),
                );
                cache
                    .category_id_to_expr
                    .insert(token.clone(), expr.clone());
                (token, eval)
            });

        let category_id = entry.0.clone();
        cache
            .collection_id_to_category_id
            .insert(collection_id.clone(), category_id.clone());
        cache
            .category_id_to_collection_ids
            .entry(category_id.clone())
            .or_default()
            .insert(collection_id.clone());
        cache.dirty_state.push((expr, Some(collection_id)));
    }

    fn remove_collection_inner(
        cache: &mut Self,
        collection_id: &CollectionId,
        invalidate_targets_and_collection: bool,
    ) {
        let category_id = match cache.collection_id_to_category_id.remove(collection_id) {
            Some(id) => id,
            None => return,
        };
        if category_id.is_empty() {
            return;
        }
        let collections_using_id = cache
            .category_id_to_collection_ids
            .get_mut(&category_id)
            .expect("entry exists");
        collections_using_id.remove(collection_id);
        let sharing_id = !collections_using_id.is_empty();
        let expr = cache
            .category_id_to_expr
            .get(&category_id)
            .expect("entry exists")
            .clone();
        if !sharing_id {
            cache.category_id_to_collection_ids.remove(&category_id);
            cache.category_id_to_expr.remove(&category_id);
            cache.expr_to_category_id_and_eval.remove(&expr.get_text());
        }
        if invalidate_targets_and_collection {
            cache.dirty_state.push((expr, Some(collection_id.clone())));
        } else {
            cache.dirty_state.push((expr, None));
        }
    }

    fn remove_collection(me: &Arc<RwLock<Self>>, prim_path: &SdfPath, collection_name: &TfToken) {
        let collection_id = (prim_path.clone(), collection_name.clone());
        let mut guard = me.write();
        let cache = &mut *guard;
        Self::remove_collection_inner(cache, &collection_id, false);
    }

    fn compute_categories_for_prim_path(&self, prim_path: &SdfPath) -> Vec<TfToken> {
        let mut categories = Vec::new();
        for (_expr_text, (category_id, eval)) in &self.expr_to_category_id_and_eval {
            if eval.match_path(prim_path).value {
                categories.push(category_id.clone());
            }
        }
        categories
    }

    fn get_category_id_for_light_linking_collection(
        &self,
        prim_path: &SdfPath,
        collection_name: &TfToken,
    ) -> Option<TfToken> {
        self.collection_id_to_category_id
            .get(&(prim_path.clone(), collection_name.clone()))
            .cloned()
    }

    fn invalidate_prims_and_clear_dirty_state(
        me: &Arc<RwLock<Self>>,
        dirtied_entries: &mut Vec<DirtiedPrimEntry>,
        populating: bool,
    ) {
        let (exprs, collection_ids) = {
            let mut guard = me.write();
            let cache = &mut *guard;
            if cache.dirty_state.is_empty() {
                return;
            }
            if populating {
                cache.dirty_state.clear();
                return;
            }
            let mut exprs_set: HashSet<String> = HashSet::new();
            let mut collection_ids_set: HashSet<CollectionId> = HashSet::new();
            for (expr, opt_col_id) in &cache.dirty_state {
                exprs_set.insert(expr.get_text());
                if let Some(cid) = opt_col_id {
                    collection_ids_set.insert(cid.clone());
                }
            }
            let combined_expr = {
                let mut combined = PathExpression::nothing();
                for expr_text in &exprs_set {
                    let parsed = PathExpression::parse(expr_text);
                    if parsed.parse_error().is_none() {
                        combined = PathExpression::make_op(
                            usd_sdf::PathExpressionOp::Union,
                            combined,
                            parsed,
                        );
                    }
                }
                combined
            };
            std::mem::take(&mut cache.dirty_state);
            (vec![combined_expr], collection_ids_set)
        };

        if exprs.is_empty() || exprs[0].is_empty() {
            return;
        }

        let combined = &exprs[0];
        if PathExpression::is_trivial(combined) {
            for cid in &collection_ids {
                let mut locators = HdDataSourceLocatorSet::new();
                locators.insert(HdLightSchema::get_default_locator().append(&cid.1));
                dirtied_entries.push(DirtiedPrimEntry {
                    prim_path: cid.0.clone(),
                    dirty_locators: locators,
                });
            }
            return;
        }

        let si = me.read().scene_index.clone();
        let eval = HdCollectionExpressionEvaluator::new(si.clone(), combined);
        let mut targets = Vec::new();
        let root = SdfPath::absolute_root();
        eval.populate_all_matches(&root, &mut targets);

        for target in &targets {
            let mut locators = HdDataSourceLocatorSet::new();
            locators.insert(HdCategoriesSchema::get_default_locator());
            dirtied_entries.push(DirtiedPrimEntry {
                prim_path: target.clone(),
                dirty_locators: locators,
            });
        }
        for cid in &collection_ids {
            let mut locators = HdDataSourceLocatorSet::new();
            locators.insert(HdLightSchema::get_default_locator().append(&cid.1));
            dirtied_entries.push(DirtiedPrimEntry {
                prim_path: cid.0.clone(),
                dirty_locators: locators,
            });
        }
    }
}

// --- Data source wrappers ---

fn is_light(prim_type: &TfToken) -> bool {
    let s = prim_type.as_str();
    s == "light"
        || s.ends_with("Light")
        || s == "sphereLight"
        || s == "domeLight"
        || s == "distantLight"
        || s == "diskLight"
        || s == "rectLight"
        || s == "cylinderLight"
        || s == "meshLight"
}

fn is_light_filter(prim_type: &TfToken) -> bool {
    let s = prim_type.as_str();
    s == "lightFilter" || s.ends_with("Filter")
}

fn is_geometry(prim_type: &TfToken) -> bool {
    let s = prim_type.as_str();
    matches!(
        s,
        "mesh"
            | "points"
            | "basisCurves"
            | "nurbsCurves"
            | "nurbsPatch"
            | "tetMesh"
            | "cube"
            | "sphere"
            | "cylinder"
            | "cone"
            | "capsule"
            | "plane"
            | "volume"
    )
}

/// Light linking scene index.
pub struct HdsiLightLinkingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    cache: Arc<RwLock<LightLinkingCache>>,
    state: Mutex<LightLinkingSceneIndexState>,
}

#[derive(Default)]
struct LightLinkingSceneIndexState {
    light_and_filter_prim_paths: HashSet<SdfPath>,
    was_populated: bool,
}

impl HdsiLightLinkingSceneIndex {
    /// Create a new light linking scene index.
    pub fn new(
        input_scene: HdSceneIndexHandle,
        _input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let cache = LightLinkingCache::new(input_scene.clone());
        let slf = Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            cache: cache.clone(),
            state: Mutex::new(LightLinkingSceneIndexState::default()),
        };
        let observer = Arc::new(RwLock::new(slf));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    fn process_added_light_or_filter(
        cache: &Arc<RwLock<LightLinkingCache>>,
        entry: &AddedPrimEntry,
        collection_names: &[TfToken],
    ) {
        let input = {
            let slf_ref = cache.read();
            slf_ref.scene_index.clone()
        };
        let prim = si_ref(&input).get_prim(&entry.prim_path);
        let collections = HdCollectionsSchema::get_from_parent(
            prim.data_source.as_ref().unwrap_or(&*EMPTY_PRIM_CONTAINER),
        );
        for col_name in collection_names {
            let col = collections.get_collection(col_name);
            if !col.is_defined() {
                continue;
            }
            if let Some(expr_ds) = col.get_membership_expression() {
                let expr = expr_ds
                    .as_ref()
                    .as_sampled()
                    .and_then(|s| s.get_value(0.0).downcast_clone::<PathExpression>())
                    .unwrap_or_default();
                if PathExpression::is_trivial(&expr) {
                    continue;
                }
                LightLinkingCache::process_collection(cache, &entry.prim_path, col_name, expr);
            }
        }
    }
}

impl HdSceneIndexBase for HdsiLightLinkingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            return HdSceneIndexPrim::default();
        };

        if prim.data_source.is_some() {
            let cache = self.cache.read();
            let prim_ds = prim.data_source.as_ref().unwrap();
            if is_geometry(&prim.prim_type) && !is_instanced(prim_ds) {
                if let Some(categories_ds) = build_categories_data_source(&cache, prim_path) {
                    let overlay_container = HdRetainedContainerDataSource::new_1(
                        (*HdCategoriesSchema::get_schema_token()).clone(),
                        categories_ds as HdDataSourceBaseHandle,
                    );
                    if let Some(overlay) = HdOverlayContainerDataSource::overlayed(
                        Some(overlay_container),
                        Some(prim_ds.clone()),
                    ) {
                        prim.data_source = Some(overlay);
                    }
                }
            } else if prim.prim_type == *tokens::INSTANCER {
                let mut overlay_entries: Vec<(TfToken, HdDataSourceBaseHandle)> = Vec::new();
                if let Some(ic_ds) =
                    build_instance_categories_data_source(&cache, prim_path, prim_ds)
                {
                    overlay_entries.push((
                        (*HdInstanceCategoriesSchema::get_schema_token()).clone(),
                        ic_ds as HdDataSourceBaseHandle,
                    ));
                }
                if let Some(dep_ds) = build_dependencies_data_source(prim_ds) {
                    overlay_entries.push((
                        (*HdDependenciesSchema::get_schema_token()).clone(),
                        dep_ds as HdDataSourceBaseHandle,
                    ));
                }
                if !overlay_entries.is_empty() {
                    let overlay_container =
                        HdRetainedContainerDataSource::from_entries(&overlay_entries);
                    if let Some(overlay) = HdOverlayContainerDataSource::overlayed(
                        Some(overlay_container),
                        Some(prim_ds.clone()),
                    ) {
                        prim.data_source = Some(overlay);
                    }
                }
            } else if is_light(&prim.prim_type) || is_light_filter(&prim.prim_type) {
                let collections = HdCollectionsSchema::get_from_parent(prim_ds);
                let light_schema = HdLightSchema::get_from_parent(prim_ds);
                let light_ds = light_schema.get_container().cloned();
                let schema_tokens = get_light_linking_schema_tokens();
                let mut overlay_entries = Vec::new();
                for token in schema_tokens {
                    let collection_name = if token == "lightFilterLink" {
                        &light_tokens::FILTER_LINK
                    } else {
                        token
                    };
                    if collections.get_collection(collection_name).is_defined() {
                        if let Some(cat_id) = cache.get_category_id_for_light_linking_collection(
                            prim_path,
                            collection_name,
                        ) {
                            overlay_entries.push((
                                token.clone(),
                                HdRetainedTypedSampledDataSource::new(cat_id)
                                    as HdDataSourceBaseHandle,
                            ));
                        } else {
                            overlay_entries.push((
                                token.clone(),
                                HdRetainedTypedSampledDataSource::new(TfToken::default())
                                    as HdDataSourceBaseHandle,
                            ));
                        }
                    }
                }
                if !overlay_entries.is_empty() {
                    let light_overlay =
                        HdRetainedContainerDataSource::from_entries(&overlay_entries);
                    let combined = if let Some(ld) = light_ds {
                        HdOverlayContainerDataSource::overlayed(
                            Some(light_overlay.clone()),
                            Some(ld),
                        )
                        .unwrap_or_else(|| light_overlay as HdContainerDataSourceHandle)
                    } else {
                        light_overlay as HdContainerDataSourceHandle
                    };
                    let light_token = (*HdLightSchema::get_schema_token()).clone();
                    let top_overlay = HdRetainedContainerDataSource::new_1(
                        light_token,
                        combined as HdDataSourceBaseHandle,
                    );
                    if let Some(overlay) = HdOverlayContainerDataSource::overlayed(
                        Some(top_overlay),
                        Some(prim_ds.clone()),
                    ) {
                        prim.data_source = Some(overlay);
                    }
                }
            }
        }

        prim
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

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiLightLinkingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiLightLinkingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.base().is_observed() {
            self.base.forward_prims_added(self, entries);
            return;
        }
        let populating = {
            let mut state = self.state.lock().expect("Lock poisoned");
            let populating = !state.was_populated;
            state.was_populated = true;
            populating
        };

        let mut dirtied_entries = Vec::new();
        for entry in entries {
            if is_light(&entry.prim_type) {
                self.state
                    .lock()
                    .expect("Lock poisoned")
                    .light_and_filter_prim_paths
                    .insert(entry.prim_path.clone());
                Self::process_added_light_or_filter(
                    &self.cache,
                    entry,
                    &[
                        light_tokens::LIGHT_LINK.clone(),
                        light_tokens::SHADOW_LINK.clone(),
                    ],
                );
            } else if is_light_filter(&entry.prim_type) {
                self.state
                    .lock()
                    .expect("Lock poisoned")
                    .light_and_filter_prim_paths
                    .insert(entry.prim_path.clone());
                Self::process_added_light_or_filter(
                    &self.cache,
                    entry,
                    &[
                        light_tokens::FILTER_LINK.clone(),
                        light_tokens::SHADOW_LINK.clone(),
                    ],
                );
            } else if self
                .state
                .lock()
                .expect("Lock poisoned")
                .light_and_filter_prim_paths
                .contains(&entry.prim_path)
            {
                for col_name in get_all_linking_collection_names() {
                    LightLinkingCache::remove_collection(&self.cache, &entry.prim_path, col_name);
                }
                self.state
                    .lock()
                    .expect("Lock poisoned")
                    .light_and_filter_prim_paths
                    .remove(&entry.prim_path);
            }
        }

        LightLinkingCache::invalidate_prims_and_clear_dirty_state(
            &self.cache,
            &mut dirtied_entries,
            populating,
        );
        self.base.forward_prims_added(self, entries);
        if !dirtied_entries.is_empty() {
            self.base.base().send_prims_dirtied(self, &dirtied_entries);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if !self.base.base().is_observed() {
            self.base.forward_prims_removed(self, entries);
            return;
        }
        let mut dirtied_entries = Vec::new();
        for entry in entries {
            let to_remove: Vec<_> = {
                let state = self.state.lock().expect("Lock poisoned");
                state
                    .light_and_filter_prim_paths
                    .iter()
                    .filter(|p| p.has_prefix(&entry.prim_path))
                    .cloned()
                    .collect()
            };
            for tracked in &to_remove {
                for col_name in get_all_linking_collection_names() {
                    LightLinkingCache::remove_collection(&self.cache, tracked, col_name);
                }
                self.state
                    .lock()
                    .expect("Lock poisoned")
                    .light_and_filter_prim_paths
                    .remove(tracked);
            }
        }
        LightLinkingCache::invalidate_prims_and_clear_dirty_state(
            &self.cache,
            &mut dirtied_entries,
            false,
        );
        self.base.forward_prims_removed(self, entries);
        if !dirtied_entries.is_empty() {
            self.base.base().send_prims_dirtied(self, &dirtied_entries);
        }
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if !self.base.base().is_observed() {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }
        let collection_locators = {
            let mut s = HdDataSourceLocatorSet::new();
            s.insert(HdCollectionsSchema::get_default_locator().append(&light_tokens::LIGHT_LINK));
            s.insert(HdCollectionsSchema::get_default_locator().append(&light_tokens::SHADOW_LINK));
            s.insert(HdCollectionsSchema::get_default_locator().append(&light_tokens::FILTER_LINK));
            s
        };

        let mut new_entries = Vec::new();
        for entry in entries {
            if !self
                .state
                .lock()
                .expect("Lock poisoned")
                .light_and_filter_prim_paths
                .contains(&entry.prim_path)
            {
                continue;
            }
            if !entry.dirty_locators.intersects(&collection_locators) {
                continue;
            }
            let input = self.base.get_input_scene().unwrap();
            let prim = si_ref(&input).get_prim(&entry.prim_path);
            let collections = HdCollectionsSchema::get_from_parent(
                prim.data_source.as_ref().unwrap_or(&*EMPTY_PRIM_CONTAINER),
            );
            if !collections.is_defined() {
                continue;
            }
            for token in [
                &light_tokens::LIGHT_LINK,
                &light_tokens::SHADOW_LINK,
                &light_tokens::FILTER_LINK,
            ] {
                let col = collections.get_collection(token);
                if !col.is_defined() {
                    continue;
                }
                if !entry
                    .dirty_locators
                    .intersects_locator(&HdCollectionsSchema::get_default_locator().append(token))
                {
                    continue;
                }
                if let Some(expr_ds) = col.get_membership_expression() {
                    let expr = expr_ds
                        .as_ref()
                        .as_sampled()
                        .and_then(|s| s.get_value(0.0).downcast_clone::<PathExpression>())
                        .unwrap_or_default();
                    LightLinkingCache::process_collection(
                        &self.cache,
                        &entry.prim_path,
                        token,
                        expr,
                    );
                }
            }
        }

        LightLinkingCache::invalidate_prims_and_clear_dirty_state(
            &self.cache,
            &mut new_entries,
            false,
        );
        self.base.forward_prims_dirtied(self, entries);
        if !new_entries.is_empty() {
            self.base.base().send_prims_dirtied(self, &new_entries);
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
