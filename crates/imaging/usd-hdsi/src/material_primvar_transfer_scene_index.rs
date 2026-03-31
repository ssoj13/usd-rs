
//! Material primvar transfer scene index.
//!
//! Transfers primvars from materials to bound geometry. Geometry primvars
//! have stronger opinion. Port of pxr/imaging/hdsi/materialPrimvarTransferSceneIndex.

use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, cast_to_container,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{
    HdDependenciesSchema, HdDependencySchemaBuilder, HdLocatorDataSourceHandle,
    HdMaterialBindingsSchema, HdPathDataSourceHandle, HdPrimvarsSchema,
    MATERIAL_BINDING_ALL_PURPOSE,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

use once_cell::sync::Lazy;

static MATERIAL_BINDINGS_TO_PRIMVARS: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("materialPrimvarTransfer_materialBindingsToPrimvars"));
static MATERIAL_PRIMVARS_TO_PRIMVARS: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("materialPrimvarTransfer_materialPrimvarsToPrimvars"));
static MATERIAL_BINDINGS_TO_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("materialPrimvarTransfer_materialBindingsToDependency"));

/// Compose function for combining geometry and material primvars.
pub type ComposeFn = Option<
    Arc<
        dyn Fn(
                &HdContainerDataSourceHandle,
                &HdContainerDataSourceHandle,
                &TfToken,
            ) -> Option<HdDataSourceBaseHandle>
            + Send
            + Sync,
    >,
>;

/// Hydra scene index that transfers primvars from materials to bound geometry.
pub struct HdsiMaterialPrimvarTransferSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    compose_fn: ComposeFn,
}

fn add_if_necessary(name: TfToken, names: &mut Vec<TfToken>) {
    if !names.iter().any(|n| *n == name) {
        names.push(name);
    }
}

fn get_material_path(material_bindings: &HdMaterialBindingsSchema) -> SdfPath {
    material_bindings
        .get_material_binding(&*MATERIAL_BINDING_ALL_PURPOSE)
        .get_path()
        .unwrap_or_default()
}

/// Primvars data source - combines geometry and material primvars.
#[derive(Clone)]
struct PrimvarsDataSource {
    input_scene: HdSceneIndexHandle,
    prim_ds: HdContainerDataSourceHandle,
    primvars_ds: Option<HdContainerDataSourceHandle>,
    compose_fn: ComposeFn,
}

impl PrimvarsDataSource {
    fn new(
        input_scene: HdSceneIndexHandle,
        prim_ds: HdContainerDataSourceHandle,
        primvars_ds: Option<HdContainerDataSourceHandle>,
        compose_fn: ComposeFn,
    ) -> Arc<Self> {
        Arc::new(Self {
            input_scene,
            prim_ds,
            primvars_ds,
            compose_fn,
        })
    }

    fn get_primvars_from_material(&self) -> Option<HdContainerDataSourceHandle> {
        let material_bindings = HdMaterialBindingsSchema::get_from_parent(&self.prim_ds);
        let material_path = get_material_path(&material_bindings);
        if material_path.is_empty() {
            return None;
        }
        let material_prim = si_ref(&self.input_scene).get_prim(&material_path);
        material_prim.data_source.as_ref().and_then(|ds| {
            let schema = HdPrimvarsSchema::get_from_parent(ds);
            schema.get_container().cloned()
        })
    }
}

impl HdContainerDataSource for PrimvarsDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        let mpds = self.get_primvars_from_material();
        if self.primvars_ds.is_none() && mpds.is_none() {
            return Vec::new();
        }
        let primvars_names = self
            .primvars_ds
            .as_ref()
            .map(|ds| ds.get_names())
            .unwrap_or_default();
        let mpds_names = mpds.as_ref().map(|ds| ds.get_names()).unwrap_or_default();
        if mpds_names.is_empty() {
            return primvars_names;
        }
        if primvars_names.is_empty() {
            return mpds_names;
        }
        let mut names: HashSet<TfToken> = primvars_names.into_iter().collect();
        names.extend(mpds_names);
        names.into_iter().collect()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if let Some(ref compose) = self.compose_fn {
            let strong = self.primvars_ds.clone().unwrap_or_else(|| {
                HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle
            });
            let weak = self.get_primvars_from_material().unwrap_or_else(|| {
                HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle
            });
            if let Some(result) = compose(&strong, &weak, name) {
                return Some(result);
            }
        }
        if let Some(ref primvars) = self.primvars_ds {
            if let Some(primvar) = primvars.get(name) {
                return Some(primvar);
            }
        }
        if let Some(mpds) = self.get_primvars_from_material() {
            return mpds.get(name);
        }
        None
    }
}

impl std::fmt::Debug for PrimvarsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimvarsDataSource").finish()
    }
}

impl HdDataSourceBase for PrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input_scene: self.input_scene.clone(),
            prim_ds: self.prim_ds.clone(),
            primvars_ds: self.primvars_ds.clone(),
            compose_fn: self.compose_fn.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

/// Prim data source - wraps input and overlays primvars + dependencies.
#[derive(Clone)]
struct PrimDataSource {
    input_scene: HdSceneIndexHandle,
    input_ds: HdContainerDataSourceHandle,
    compose_fn: ComposeFn,
}

impl PrimDataSource {
    fn new(
        input_scene: HdSceneIndexHandle,
        input_ds: HdContainerDataSourceHandle,
        compose_fn: ComposeFn,
    ) -> Arc<Self> {
        Arc::new(Self {
            input_scene,
            input_ds,
            compose_fn,
        })
    }

    fn get_dependencies(&self) -> Option<HdContainerDataSourceHandle> {
        let material_bindings = HdMaterialBindingsSchema::get_from_parent(&self.input_ds);
        if !material_bindings.is_defined() {
            return None;
        }
        let material_bindings_loc = HdMaterialBindingsSchema::get_default_locator();
        let primvars_loc = HdPrimvarsSchema::get_default_locator();
        let material_bindings_loc_ds = HdRetainedTypedSampledDataSource::new(material_bindings_loc)
            as HdLocatorDataSourceHandle;
        let primvars_loc_ds =
            HdRetainedTypedSampledDataSource::new(primvars_loc) as HdLocatorDataSourceHandle;
        let mut names: Vec<TfToken> = Vec::new();
        let mut data_sources: Vec<HdDataSourceBaseHandle> = Vec::new();
        {
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_data_source_locator(material_bindings_loc_ds.clone())
                .set_affected_data_source_locator(primvars_loc_ds.clone())
                .build();
            names.push((*MATERIAL_BINDINGS_TO_PRIMVARS).clone());
            data_sources.push(dep as HdDataSourceBaseHandle);
        }
        let material_path = get_material_path(&material_bindings);
        if !material_path.is_empty() {
            let path_ds =
                HdRetainedTypedSampledDataSource::new(material_path) as HdPathDataSourceHandle;
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_prim_path(path_ds)
                .set_depended_on_data_source_locator(primvars_loc_ds.clone())
                .set_affected_data_source_locator(primvars_loc_ds.clone())
                .build();
            names.push((*MATERIAL_PRIMVARS_TO_PRIMVARS).clone());
            data_sources.push(dep as HdDataSourceBaseHandle);
        }
        {
            let dep_loc =
                HdDependenciesSchema::get_default_locator().append(&*MATERIAL_PRIMVARS_TO_PRIMVARS);
            let dep_loc_ds =
                HdRetainedTypedSampledDataSource::new(dep_loc) as HdLocatorDataSourceHandle;
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_data_source_locator(material_bindings_loc_ds.clone())
                .set_affected_data_source_locator(dep_loc_ds)
                .build();
            names.push((*MATERIAL_BINDINGS_TO_DEPENDENCY).clone());
            data_sources.push(dep as HdDataSourceBaseHandle);
        }
        let entries: Vec<(TfToken, HdDataSourceBaseHandle)> =
            names.into_iter().zip(data_sources.into_iter()).collect();
        Some(HdRetainedContainerDataSource::from_entries(&entries))
    }
}

impl HdContainerDataSource for PrimDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        let mut result = self.input_ds.get_names();
        let material_bindings = HdMaterialBindingsSchema::get_from_parent(&self.input_ds);
        if material_bindings.is_defined() {
            add_if_necessary(
                (*HdDependenciesSchema::get_schema_token()).clone(),
                &mut result,
            );
            if !get_material_path(&material_bindings).is_empty() {
                add_if_necessary((*HdPrimvarsSchema::get_schema_token()).clone(), &mut result);
            }
        }
        result
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let ds = self.input_ds.get(name);
        if *name == **HdPrimvarsSchema::get_schema_token() {
            let primvars_ds = ds.as_ref().and_then(|d| cast_to_container(d));
            return Some(PrimvarsDataSource::new(
                self.input_scene.clone(),
                self.input_ds.clone(),
                primvars_ds,
                self.compose_fn.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == **HdDependenciesSchema::get_schema_token() {
            if let Some(deps) = self.get_dependencies() {
                let existing = ds.as_ref().and_then(|d| cast_to_container(d));
                if let Some(overlay) = HdOverlayContainerDataSource::overlayed(Some(deps), existing)
                {
                    return Some(overlay as HdDataSourceBaseHandle);
                }
            }
        }
        ds
    }
}

impl std::fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDataSource").finish()
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input_scene: self.input_scene.clone(),
            input_ds: self.input_ds.clone(),
            compose_fn: self.compose_fn.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

use usd_hd::data_source::HdOverlayContainerDataSource;

impl HdsiMaterialPrimvarTransferSceneIndex {
    /// Default compose: geometry primvars (strong) override material primvars (weak).
    pub fn default_compose_fn(
        ds_strong: &HdContainerDataSourceHandle,
        ds_weak: &HdContainerDataSourceHandle,
        name: &TfToken,
    ) -> Option<HdDataSourceBaseHandle> {
        if let Some(v) = ds_strong.get(name) {
            return Some(v);
        }
        ds_weak.get(name)
    }

    /// Creates a new material primvar transfer scene index.
    pub fn new(input_scene: HdSceneIndexHandle, compose_fn: ComposeFn) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            compose_fn,
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

impl HdSceneIndexBase for HdsiMaterialPrimvarTransferSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };
        if prim.data_source.is_some() {
            if let Some(input) = self.base.get_input_scene() {
                prim.data_source = Some(PrimDataSource::new(
                    input.clone(),
                    prim.data_source.clone().unwrap(),
                    self.compose_fn.clone(),
                ) as HdContainerDataSourceHandle);
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

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiMaterialPrimvarTransferSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiMaterialPrimvarTransferSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if entries.len() >= 500 {
            let first_path = entries
                .first()
                .map(|entry| entry.prim_path.to_string())
                .unwrap_or_else(|| "<none>".to_string());
            eprintln!(
                "[material_primvar_transfer] on_prims_dirtied in={} sender={} first={}",
                entries.len(),
                sender.get_display_name(),
                first_path
            );
        }
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
