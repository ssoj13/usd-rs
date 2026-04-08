//! Base scene index for material network filtering.
//!
//! Port of pxr/imaging/hd/materialFilteringSceneIndexBase.{h,cpp}

use super::filtering::{FilteringObserverTarget, HdSingleInputFilteringSceneIndexBase};
use super::observer::{AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry};
use crate::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    cast_to_container,
};
use crate::data_source_material_network_interface::HdDataSourceMaterialNetworkInterface;
use crate::material_network_interface::HdMaterialNetworkInterface;
use crate::scene_index::observer::HdSceneIndexObserverHandle;
use crate::schema::MATERIAL;
use crate::tokens::SPRIM_MATERIAL;
use crate::{HdSceneIndexBase, HdSceneIndexPrim, si_ref};
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Callback type for filtering material networks.
pub type MaterialFilteringFn = Arc<dyn Fn(&mut dyn HdMaterialNetworkInterface) + Send + Sync>;

/// Container that wraps material networks and runs the filtering callback when accessed.
struct MaterialDataSource {
    material_input: HdContainerDataSourceHandle,
    prim_input: HdContainerDataSourceHandle,
    prim_path: SdfPath,
    filtering_fn: MaterialFilteringFn,
}

impl MaterialDataSource {
    fn new(
        material_input: HdContainerDataSourceHandle,
        prim_input: HdContainerDataSourceHandle,
        prim_path: SdfPath,
        filtering_fn: MaterialFilteringFn,
    ) -> Arc<Self> {
        Arc::new(Self {
            material_input,
            prim_input,
            prim_path,
            filtering_fn,
        })
    }
}

impl HdDataSourceBase for MaterialDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(MaterialDataSource {
            material_input: self.material_input.clone(),
            prim_input: self.prim_input.clone(),
            prim_path: self.prim_path.clone(),
            filtering_fn: self.filtering_fn.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(MaterialDataSource {
            material_input: self.material_input.clone(),
            prim_input: self.prim_input.clone(),
            prim_path: self.prim_path.clone(),
            filtering_fn: self.filtering_fn.clone(),
        }))
    }
}

impl HdContainerDataSource for MaterialDataSource {
    fn get_names(&self) -> Vec<Token> {
        self.material_input.get_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let result = self.material_input.get(name)?;
        let network_container = cast_to_container(&result)?;

        let mut network_interface = HdDataSourceMaterialNetworkInterface::new(
            self.prim_path.clone(),
            network_container,
            Some(self.prim_input.clone()),
        );
        (self.filtering_fn)(&mut network_interface);
        let finished = network_interface.finish();
        Some(finished as HdDataSourceBaseHandle)
    }
}

impl std::fmt::Debug for MaterialDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialDataSource")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

/// Container that wraps prim data source and intercepts material for filtering.
struct PrimDataSource {
    base: Arc<dyn std::any::Any + Send + Sync>,
    prim_input: HdContainerDataSourceHandle,
    prim_path: SdfPath,
    filtering_fn: MaterialFilteringFn,
}

impl PrimDataSource {
    fn new(
        base: Arc<dyn std::any::Any + Send + Sync>,
        prim_input: HdContainerDataSourceHandle,
        prim_path: SdfPath,
        filtering_fn: MaterialFilteringFn,
    ) -> Arc<Self> {
        Arc::new(Self {
            base,
            prim_input,
            prim_path,
            filtering_fn,
        })
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrimDataSource {
            base: self.base.clone(),
            prim_input: self.prim_input.clone(),
            prim_path: self.prim_path.clone(),
            filtering_fn: self.filtering_fn.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(PrimDataSource {
            base: self.base.clone(),
            prim_input: self.prim_input.clone(),
            prim_path: self.prim_path.clone(),
            filtering_fn: self.filtering_fn.clone(),
        }))
    }
}

impl HdContainerDataSource for PrimDataSource {
    fn get_names(&self) -> Vec<Token> {
        self.prim_input.get_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let result = self.prim_input.get(name)?;
        if name == &*MATERIAL {
            let material_container = cast_to_container(&result)?;
            let material_ds = MaterialDataSource::new(
                material_container,
                self.prim_input.clone(),
                self.prim_path.clone(),
                self.filtering_fn.clone(),
            );
            return Some(material_ds as HdDataSourceBaseHandle);
        }
        Some(result)
    }
}

impl std::fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDataSource")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

/// Base for scene indices that filter material network data sources.
///
/// Subclasses implement `get_filtering_function` to provide a callback
/// that runs when a material network is first queried.
pub struct HdMaterialFilteringSceneIndexBase {
    base: HdSingleInputFilteringSceneIndexBase,
    filtering_fn: MaterialFilteringFn,
}

impl HdMaterialFilteringSceneIndexBase {
    /// Creates a new material filtering scene index.
    ///
    /// NOTE: Since this is an embeddable base (returns Self, not Arc<RwLock<Self>>),
    /// the caller must wire the observer after wrapping in Arc<RwLock<>>. Use
    /// `super::filtering::wire_filter_to_input` on the outer type.
    pub fn new(
        input_scene: Option<crate::scene_index::base::HdSceneIndexHandle>,
        filtering_fn: MaterialFilteringFn,
    ) -> Self {
        Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            filtering_fn,
        }
    }

    /// Returns a clone of the input scene handle for observer wiring by the embedding type.
    pub fn get_input_scene_handle(&self) -> Option<crate::scene_index::base::HdSceneIndexHandle> {
        self.base.get_input_scene().cloned()
    }

    /// Returns the filtering function.
    pub fn get_filtering_function(&self) -> &MaterialFilteringFn {
        &self.filtering_fn
    }

    /// Access the underlying single-input filtering base.
    pub fn base(&self) -> &HdSingleInputFilteringSceneIndexBase {
        &self.base
    }

    /// Mutable access to the base.
    pub fn base_mut(&mut self) -> &mut HdSingleInputFilteringSceneIndexBase {
        &mut self.base
    }

    /// Forward prims added from this filtering view.
    pub fn forward_prims_added(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[AddedPrimEntry],
    ) {
        self.base.forward_prims_added(scene_index, entries);
    }

    /// Forward prims removed.
    pub fn forward_prims_removed(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[RemovedPrimEntry],
    ) {
        self.base.forward_prims_removed(scene_index, entries);
    }

    /// Forward prims dirtied.
    pub fn forward_prims_dirtied(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[DirtiedPrimEntry],
    ) {
        self.base.forward_prims_dirtied(scene_index, entries);
    }

    /// Forward prims renamed.
    pub fn forward_prims_renamed(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[RenamedPrimEntry],
    ) {
        self.base.forward_prims_renamed(scene_index, entries);
    }
}

impl HdSceneIndexBase for HdMaterialFilteringSceneIndexBase {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if prim.prim_type == *SPRIM_MATERIAL {
            if let Some(prim_container) = &prim.data_source {
                let filtering_fn = self.filtering_fn.clone();
                prim.data_source = Some(PrimDataSource::new(
                    Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
                    prim_container.clone(),
                    prim_path.clone(),
                    filtering_fn,
                ) as HdContainerDataSourceHandle);
            }
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> Vec<SdfPath> {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_locked = input.read();
                return input_locked.get_child_prim_paths(prim_path);
            }
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
        "HdMaterialFilteringSceneIndexBase".to_string()
    }
}

impl FilteringObserverTarget for HdMaterialFilteringSceneIndexBase {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.forward_prims_renamed(self, entries);
    }
}
