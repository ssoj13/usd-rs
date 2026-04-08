//! Render settings filtering scene index.
//!
//! A filtering scene index that manages render settings prims: filters
//! namespacedSettings by prefix, adds computed active/shutterInterval,
//! and registers dependencies. Matches C++ HdsiRenderSettingsFilteringSceneIndex.

use crate::tokens as hdsi_tokens;
use parking_lot::RwLock;
use std::fmt;
use std::sync::Arc;
use usd_gf::Vec2d;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
    cast_to_container,
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
    HdCameraSchema, HdDependenciesSchema, HdDependencySchemaBuilder, HdPathDataSourceHandle,
    HdRenderProductSchema, HdRenderSettingsSchema, HdSceneGlobalsSchema, RENDER_SETTINGS_ACTIVE,
    RENDER_SETTINGS_NAMESPACED, RENDER_SETTINGS_SHUTTER_INTERVAL, RENDER_SETTINGS_TOKEN,
};
use usd_hd::tokens;
use usd_hd::utils;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Render scope path: /Render
const RENDER_SCOPE: &str = "/Render";
/// Fallback settings path
const FALLBACK_PATH: &str = "/Render/__HdsiRenderSettingsFilteringSceneIndex__FallbackSettings";

fn get_render_scope() -> SdfPath {
    SdfPath::from_string(RENDER_SCOPE).unwrap()
}

fn get_fallback_path() -> SdfPath {
    SdfPath::from_string(FALLBACK_PATH).unwrap()
}

/// Get namespace prefixes from input args.
fn get_namespace_prefixes(input_args: Option<&HdContainerDataSourceHandle>) -> Vec<TfToken> {
    let input_args = match input_args {
        Some(a) => a,
        None => return Vec::new(),
    };

    let child = match input_args
        .get(&hdsi_tokens::RENDER_SETTINGS_FILTERING_SCENE_INDEX_TOKENS.namespace_prefixes)
    {
        Some(c) => c,
        None => return Vec::new(),
    };

    if let Some(value) = child.sample_at_zero() {
        if let Some(tokens) = value.get::<Vec<TfToken>>() {
            return tokens.clone();
        }
    }

    Vec::new()
}

/// Get fallback prim data source from input args.
fn get_fallback_prim_ds(
    input_args: Option<&HdContainerDataSourceHandle>,
) -> Option<HdContainerDataSourceHandle> {
    let input_args = input_args?;
    let child = input_args
        .get(&hdsi_tokens::RENDER_SETTINGS_FILTERING_SCENE_INDEX_TOKENS.fallback_prim_ds)?;
    cast_to_container(&child)
}

/// Filter namespaced settings to only include names starting with prefixes.
fn get_filtered_namespaced_settings(
    c: Option<&HdContainerDataSourceHandle>,
    prefixes: &[TfToken],
) -> Option<HdContainerDataSourceHandle> {
    let c = c?;
    if prefixes.is_empty() {
        return Some(c.clone());
    }

    let names: Vec<_> = c
        .get_names()
        .into_iter()
        .filter(|name| {
            let s = name.as_str();
            prefixes.iter().any(|p| s.starts_with(p.as_str()))
        })
        .collect();

    if names.is_empty() {
        return Some(HdRetainedContainerDataSource::new_empty());
    }

    let entries: Vec<_> = names
        .iter()
        .filter_map(|n| c.get(n).map(|v| (n.clone(), v)))
        .collect();

    Some(HdRetainedContainerDataSource::from_entries(&entries))
}

/// Get unique camera paths from render products.
fn get_targeted_cameras(
    products_vector: Option<&usd_hd::data_source::HdVectorDataSourceHandle>,
) -> Vec<SdfPath> {
    let vec_ds = match products_vector {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut camera_paths = Vec::new();
    let n = vec_ds.get_num_elements();

    for i in 0..n {
        if let Some(elem) = vec_ds.get_element(i) {
            if let Some(container) = cast_to_container(&elem) {
                let product = HdRenderProductSchema::new(container);
                if let Some(cam_ds) = product.get_camera_prim() {
                    let cam_path = cam_ds.get_typed_value(0.0f32);
                    if !cam_path.is_empty() && !camera_paths.contains(&cam_path) {
                        camera_paths.push(cam_path);
                    }
                }
            }
        }
    }

    camera_paths
}

#[derive(Clone)]
struct ProductShutterInfo {
    camera_path: SdfPath,
    disable_motion_blur: bool,
}

/// Get shutter info from products.
fn get_shutter_info_from_products(
    products_vector: Option<&usd_hd::data_source::HdVectorDataSourceHandle>,
) -> Vec<ProductShutterInfo> {
    let vec_ds = match products_vector {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    let n = vec_ds.get_num_elements();

    for i in 0..n {
        if let Some(elem) = vec_ds.get_element(i) {
            if let Some(container) = cast_to_container(&elem) {
                let product = HdRenderProductSchema::new(container);
                if let Some(cam_ds) = product.get_camera_prim() {
                    let cam_path = cam_ds.get_typed_value(0.0f32);
                    if !cam_path.is_empty() {
                        let disable_motion_blur = product
                            .get_disable_motion_blur()
                            .map(|ds| ds.get_typed_value(0.0f32))
                            .unwrap_or(false);
                        result.push(ProductShutterInfo {
                            camera_path: cam_path,
                            disable_motion_blur,
                        });
                    }
                }
            }
        }
    }

    result
}

/// Get camera shutter open/close from scene index.
fn get_camera_shutter_open_and_close(
    si: &HdSceneIndexHandle,
    camera_path: &SdfPath,
) -> Option<Vec2d> {
    let guard = si.read();
    let prim = guard.get_prim(camera_path);
    let ds = prim.data_source.as_ref()?;
    let cam_schema = HdCameraSchema::get_from_parent(ds);
    if !cam_schema.is_defined() {
        return None;
    }

    let shutter_open = cam_schema.get_shutter_open()?;
    let shutter_close = cam_schema.get_shutter_close()?;

    let open = shutter_open.get_typed_value(0.0f32);
    let close = shutter_close.get_typed_value(0.0f32);

    Some(Vec2d::new(open, close))
}

/// Compute union of camera shutter intervals.
fn compute_unioned_camera_shutter_interval(
    si: &HdSceneIndexHandle,
    shutter_info: &[ProductShutterInfo],
) -> Option<HdDataSourceBaseHandle> {
    let mut result = Vec2d::new(0.0, 0.0);
    let mut initialized = false;

    for info in shutter_info {
        let mut cam_shutter = match get_camera_shutter_open_and_close(si, &info.camera_path) {
            Some(s) => s,
            None => continue,
        };

        if info.disable_motion_blur {
            cam_shutter = Vec2d::new(0.0, 0.0);
        }

        if !initialized {
            result = cam_shutter;
            initialized = true;
        } else {
            result[0] = result[0].min(cam_shutter[0]);
            result[1] = result[1].max(cam_shutter[1]);
        }
    }

    if initialized {
        let ds = HdRetainedTypedSampledDataSource::new(result);
        Some(ds.as_ref().clone_box())
    } else {
        None
    }
}

/// Build dependency for active locator.
fn build_dependency_for_active_locator() -> HdContainerDataSourceHandle {
    HdRetainedContainerDataSource::from_entries(&[(
        TfToken::new("active_depOn_sceneGlobals_arsp"),
        HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::<SdfPath>::new(
                HdSceneGlobalsSchema::get_default_prim_path(),
            ) as HdPathDataSourceHandle)
            .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                HdSceneGlobalsSchema::get_active_render_settings_prim_locator(),
            ))
            .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                HdRenderSettingsSchema::get_active_locator(),
            ))
            .build() as HdDataSourceBaseHandle,
    )])
}

/// Build dependency for frame locator.
fn build_dependency_for_frame_locator() -> HdContainerDataSourceHandle {
    HdRetainedContainerDataSource::from_entries(&[(
        TfToken::new("active_depOn_sceneGlobals_frame"),
        HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::<SdfPath>::new(
                HdSceneGlobalsSchema::get_default_prim_path(),
            ) as HdPathDataSourceHandle)
            .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                HdSceneGlobalsSchema::get_current_frame_locator(),
            ))
            .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                HdRenderSettingsSchema::get_frame_locator(),
            ))
            .build() as HdDataSourceBaseHandle,
    )])
}

/// Build dependencies for shutter interval (cameras + render products).
fn build_dependencies_for_shutter_interval(
    camera_paths: &[SdfPath],
) -> HdContainerDataSourceHandle {
    let shutter_open_locator = HdCameraSchema::get_shutter_open_locator();
    let shutter_close_locator = HdCameraSchema::get_shutter_close_locator();
    let shutter_interval_locator = HdRenderSettingsSchema::get_shutter_interval_locator();
    let products_locator = HdRenderSettingsSchema::get_render_products_locator();

    let mut entries: Vec<(TfToken, HdDataSourceBaseHandle)> = Vec::new();

    for (ii, cam_path) in camera_paths.iter().enumerate() {
        let dep_open = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::<SdfPath>::new(
                cam_path.clone(),
            ) as HdPathDataSourceHandle)
            .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                shutter_open_locator.clone(),
            ))
            .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                shutter_interval_locator.clone(),
            ))
            .build();
        entries.push((
            TfToken::new(&format!("renderSettings_depOn_cameraShutterOpen_{}", ii)),
            dep_open as HdDataSourceBaseHandle,
        ));

        let dep_close = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::<SdfPath>::new(
                cam_path.clone(),
            ) as HdPathDataSourceHandle)
            .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                shutter_close_locator.clone(),
            ))
            .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                shutter_interval_locator.clone(),
            ))
            .build();
        entries.push((
            TfToken::new(&format!("renderSettings_depOn_cameraShutterClose_{}", ii)),
            dep_close as HdDataSourceBaseHandle,
        ));
    }

    let dep_products = HdDependencySchemaBuilder::default()
        .set_depended_on_prim_path(
            HdRetainedTypedSampledDataSource::<SdfPath>::new(SdfPath::empty())
                as HdPathDataSourceHandle,
        )
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            products_locator.clone(),
        ))
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
            shutter_interval_locator.clone(),
        ))
        .build();
    entries.push((
        TfToken::new("shutterInterval_depOn_renderProducts"),
        dep_products as HdDataSourceBaseHandle,
    ));

    let dep_deps_on_products = HdDependencySchemaBuilder::default()
        .set_depended_on_prim_path(
            HdRetainedTypedSampledDataSource::<SdfPath>::new(SdfPath::empty())
                as HdPathDataSourceHandle,
        )
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            products_locator,
        ))
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdDependenciesSchema::get_default_locator(),
        ))
        .build();
    entries.push((
        TfToken::new("__dependencies_depOn_renderProducts"),
        dep_deps_on_products as HdDataSourceBaseHandle,
    ));

    HdRetainedContainerDataSource::from_entries(&entries)
}

/// Render settings data source: overlays active, shutterInterval, filters namespacedSettings.
#[derive(Clone)]
struct RenderSettingsDataSource {
    input: HdContainerDataSourceHandle,
    si: HdSceneIndexHandle,
    prim_path: SdfPath,
    namespace_prefixes: Vec<TfToken>,
}

impl fmt::Debug for RenderSettingsDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderSettingsDataSource")
            .field("prim_path", &self.prim_path)
            .field("namespace_prefixes", &self.namespace_prefixes)
            .finish_non_exhaustive()
    }
}

impl HdDataSourceBase for RenderSettingsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(RenderSettingsDataSource {
            input: self.input.clone(),
            si: self.si.clone(),
            prim_path: self.prim_path.clone(),
            namespace_prefixes: self.namespace_prefixes.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for RenderSettingsDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        let mut names = self.input.get_names();
        if !names.contains(&*RENDER_SETTINGS_ACTIVE) {
            names.push((*RENDER_SETTINGS_ACTIVE).clone());
        }
        if !names.contains(&*RENDER_SETTINGS_SHUTTER_INTERVAL) {
            names.push((*RENDER_SETTINGS_SHUTTER_INTERVAL).clone());
        }
        names
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if *name == *RENDER_SETTINGS_ACTIVE {
            let is_active = utils::has_active_render_settings_prim(&self.si)
                .map(|p| p == self.prim_path)
                .unwrap_or(false);
            return Some(HdRetainedTypedSampledDataSource::new(is_active) as HdDataSourceBaseHandle);
        }

        if *name == *RENDER_SETTINGS_SHUTTER_INTERVAL {
            let rs_schema = HdRenderSettingsSchema::new(self.input.clone());
            let products_vec = rs_schema.get_render_products_vector();
            let shutter_info = get_shutter_info_from_products(products_vec.as_ref());
            if let Some(ds) = compute_unioned_camera_shutter_interval(&self.si, &shutter_info) {
                return Some(ds);
            }
        }

        let mut result = self.input.get(name);

        if *name == *RENDER_SETTINGS_NAMESPACED && !self.namespace_prefixes.is_empty() {
            if let Some(ref container) = result {
                if let Some(filtered) = get_filtered_namespaced_settings(
                    cast_to_container(container).as_ref(),
                    &self.namespace_prefixes,
                ) {
                    result = Some(filtered as HdDataSourceBaseHandle);
                }
            }
        }

        result
    }
}

/// Prim data source wrapper: overlays renderSettings and __dependencies.
#[derive(Clone)]
struct RenderSettingsPrimDataSource {
    input: HdContainerDataSourceHandle,
    si: HdSceneIndexHandle,
    prim_path: SdfPath,
    namespace_prefixes: Vec<TfToken>,
}

impl fmt::Debug for RenderSettingsPrimDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderSettingsPrimDataSource")
            .field("prim_path", &self.prim_path)
            .field("namespace_prefixes", &self.namespace_prefixes)
            .finish_non_exhaustive()
    }
}

impl HdDataSourceBase for RenderSettingsPrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(RenderSettingsPrimDataSource {
            input: self.input.clone(),
            si: self.si.clone(),
            prim_path: self.prim_path.clone(),
            namespace_prefixes: self.namespace_prefixes.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for RenderSettingsPrimDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        let mut names = self.input.get_names();
        let deps = TfToken::new("__dependencies");
        if !names.contains(&deps) {
            names.push(deps);
        }
        names
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let dependencies = TfToken::new("__dependencies");

        let mut result = self.input.get(name);

        if *name == *RENDER_SETTINGS_TOKEN {
            if let Some(ref container) = result {
                if let Some(rs_container) = cast_to_container(container) {
                    let rs_ds = Arc::new(RenderSettingsDataSource {
                        input: rs_container,
                        si: self.si.clone(),
                        prim_path: self.prim_path.clone(),
                        namespace_prefixes: self.namespace_prefixes.clone(),
                    });
                    result = Some(rs_ds as HdDataSourceBaseHandle);
                }
            }
        }

        if *name == dependencies {
            let rs_schema = HdRenderSettingsSchema::get_from_parent(&self.input);
            let products_vec = rs_schema.get_render_products_vector();
            let camera_paths = get_targeted_cameras(products_vec.as_ref());

            let active_dep = build_dependency_for_active_locator();
            let shutter_deps = build_dependencies_for_shutter_interval(&camera_paths);
            let frame_dep = build_dependency_for_frame_locator();
            let existing = result.and_then(|r| cast_to_container(&r));

            let overlayed = if let Some(ex) = existing {
                HdOverlayContainerDataSource::new_4(active_dep, shutter_deps, frame_dep, ex)
            } else {
                HdOverlayContainerDataSource::new_3(active_dep, shutter_deps, frame_dep)
            };
            result = Some(overlayed as HdDataSourceBaseHandle);
        }

        result
    }
}

fn contains_path(paths: &[SdfPath], path: &SdfPath) -> bool {
    paths.contains(path)
}

/// Render settings filtering scene index.
pub struct HdsiRenderSettingsFilteringSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    namespace_prefixes: Vec<TfToken>,
    fallback_prim_ds: Option<HdContainerDataSourceHandle>,
}

impl HdsiRenderSettingsFilteringSceneIndex {
    /// Creates a new render settings filtering scene index.
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let namespace_prefixes = get_namespace_prefixes(input_args.as_ref());
        let fallback_prim_ds = get_fallback_prim_ds(input_args.as_ref());

        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            namespace_prefixes,
            fallback_prim_ds,
        }));
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

    /// Get the fallback prim path.
    pub fn get_fallback_prim_path() -> SdfPath {
        get_fallback_path()
    }

    /// Get the render scope path.
    pub fn get_render_scope() -> SdfPath {
        get_render_scope()
    }
}

impl HdSceneIndexBase for HdsiRenderSettingsFilteringSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };

        let prim = si_ref(&input).get_prim(prim_path);

        if prim.prim_type == *tokens::SPRIM_RENDER_SETTINGS {
            if let Some(ref ds) = prim.data_source {
                let wrapped = Arc::new(RenderSettingsPrimDataSource {
                    input: ds.clone(),
                    si: input.clone(),
                    prim_path: prim_path.clone(),
                    namespace_prefixes: self.namespace_prefixes.clone(),
                });
                return HdSceneIndexPrim {
                    prim_type: prim.prim_type.clone(),
                    data_source: Some(wrapped as HdContainerDataSourceHandle),
                };
            }
        }

        if *prim_path == get_fallback_path() {
            return HdSceneIndexPrim {
                prim_type: TfToken::new(tokens::SPRIM_RENDER_SETTINGS.as_str()),
                data_source: self.fallback_prim_ds.clone(),
            };
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return Vec::new(),
        };

        let guard = input.read();

        if prim_path.is_absolute_root_path() {
            let mut paths = guard.get_child_prim_paths(prim_path);
            let render_scope = get_render_scope();
            if !contains_path(&paths, &render_scope) {
                paths.push(render_scope);
            }
            return paths;
        }

        if *prim_path == get_render_scope() {
            let mut paths = guard.get_child_prim_paths(prim_path);
            paths.push(get_fallback_path());
            return paths;
        }

        guard.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiRenderSettingsFilteringSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiRenderSettingsFilteringSceneIndex {
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
