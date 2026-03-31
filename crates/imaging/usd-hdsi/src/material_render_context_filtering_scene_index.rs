
//! Material render context filtering scene index.
//!
//! Filters material networks to the first matching render context in priority order.
//! Port of pxr/imaging/hdsi/materialRenderContextFilteringSceneIndex.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    cast_to_container,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::HdMaterialSchema;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Type predicate: returns true if filtering should apply to this prim type.
pub type TypePredicateFn = Option<Arc<dyn Fn(&TfToken) -> bool + Send + Sync>>;

/// Filters material networks to a single render context by priority.
pub struct HdsiMaterialRenderContextFilteringSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    render_context_priority_order: Vec<TfToken>,
    type_predicate_fn: TypePredicateFn,
}

/// Material data source - exposes only the winning render context.
#[derive(Clone)]
struct MaterialDataSource {
    material_container: HdContainerDataSourceHandle,
    winning_context: Option<TfToken>,
}

impl MaterialDataSource {
    fn find_winning_context(
        material_container: &HdContainerDataSourceHandle,
        render_context_priority_order: &[TfToken],
    ) -> Option<TfToken> {
        let schema = HdMaterialSchema::new(material_container.clone());
        for context in render_context_priority_order {
            if schema.get_material_network_for_context(context).is_some() {
                return Some(context.clone());
            }
        }
        None
    }

    fn new(
        material_container: HdContainerDataSourceHandle,
        render_context_priority_order: &[TfToken],
    ) -> Arc<Self> {
        let winning_context =
            Self::find_winning_context(&material_container, render_context_priority_order);
        Arc::new(Self {
            material_container,
            winning_context,
        })
    }
}

impl HdContainerDataSource for MaterialDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        if let Some(ref ctx) = self.winning_context {
            return vec![ctx.clone()];
        }
        Vec::new()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if self.winning_context.as_ref() == Some(name) {
            return self.material_container.get(name);
        }
        None
    }
}

impl HdDataSourceBase for MaterialDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            material_container: self.material_container.clone(),
            winning_context: self.winning_context.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl std::fmt::Debug for MaterialDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialDataSource").finish()
    }
}

/// Prim data source - wraps material at material locator.
#[derive(Clone)]
struct PrimDataSource {
    prim_container: HdContainerDataSourceHandle,
    render_context_priority_order: Vec<TfToken>,
}

impl PrimDataSource {
    fn new(
        prim_container: HdContainerDataSourceHandle,
        render_context_priority_order: Vec<TfToken>,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim_container,
            render_context_priority_order,
        })
    }
}

impl HdContainerDataSource for PrimDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.prim_container.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if *name == **HdMaterialSchema::get_schema_token() {
            if let Some(mat_container) = self
                .prim_container
                .get(name)
                .and_then(|d| cast_to_container(&d))
            {
                return Some(MaterialDataSource::new(
                    mat_container,
                    &self.render_context_priority_order,
                ) as HdDataSourceBaseHandle);
            }
        }
        self.prim_container.get(name)
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_container: self.prim_container.clone(),
            render_context_priority_order: self.render_context_priority_order.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl std::fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDataSource").finish()
    }
}

impl HdsiMaterialRenderContextFilteringSceneIndex {
    /// Creates a new material render context filtering scene index.
    ///
    /// # Arguments
    /// * `input_scene` - Input scene index to filter
    /// * `render_context_priority_order` - Render contexts in descending preference order
    /// * `type_predicate_fn` - If Some, only filter prims where predicate returns true
    pub fn new(
        input_scene: HdSceneIndexHandle,
        render_context_priority_order: Vec<TfToken>,
        type_predicate_fn: TypePredicateFn,
    ) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            render_context_priority_order,
            type_predicate_fn,
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }
}

impl HdSceneIndexBase for HdsiMaterialRenderContextFilteringSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };
        let apply_filter = match &self.type_predicate_fn {
            None => true,
            Some(pred) => pred(&prim.prim_type),
        };
        if prim.data_source.is_some() && apply_filter {
            prim.data_source = Some(PrimDataSource::new(
                prim.data_source.clone().unwrap(),
                self.render_context_priority_order.clone(),
            ) as HdContainerDataSourceHandle);
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
        "HdsiMaterialRenderContextFilteringSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiMaterialRenderContextFilteringSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
