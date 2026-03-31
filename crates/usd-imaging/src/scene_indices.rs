//! UsdImaging scene indices factory.
//!
//! Port of pxr/usdImaging/usdImaging/sceneIndices.h/cpp
//!
//! Creates the full scene index chain for consuming a UsdStage, including
//! plugin scene indices (e.g. UsdSkelImaging).

use super::{
    draw_mode_scene_index::create_draw_mode_scene_index,
    extent_resolving_scene_index::create_extent_resolving_scene_index,
    instance_proxy_path_translation_scene_index::InstanceProxyPathTranslationSceneIndex,
    material_bindings_resolving_scene_index::create_material_bindings_resolving_scene_index,
    ni_prototype_propagating_scene_index::UsdImagingNiPrototypePropagatingSceneIndex,
    pi_prototype_propagating_scene_index::PiPrototypePropagatingSceneIndex,
    render_settings_flattening_scene_index::RenderSettingsFlatteningSceneIndex,
    selection_scene_index::SelectionSceneIndex,
    unloaded_draw_mode_scene_index::HdUnloadedDrawModeSceneIndex,
};
use crate::scene_index_plugin::UsdImagingSceneIndexPluginRegistry;
use crate::stage_scene_index::StageSceneIndex;
use crate::tokens::UsdImagingTokens;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_core::Stage;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdOverlayContainerDataSource, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::{
    HdNoticeBatchingSceneIndex, HdSceneIndexHandle, arc_scene_index_to_handle,
    hd_make_encapsulating_scene_index, scene_index_to_handle,
};
use usd_hd::schema::HdPurposeSchema;
use usd_tf::Token;
use usd_tf::getenv::tf_getenv_bool;

/// Callback to insert scene indices after the stage scene index.
///
/// Port of UsdImagingSceneIndexAppendCallback / overridesSceneIndexCallback.
pub type OverridesSceneIndexCallback =
    Option<Box<dyn Fn(HdSceneIndexHandle) -> HdSceneIndexHandle + Send + Sync>>;

/// Information for creating the UsdImaging scene index chain.
///
/// Port of UsdImagingCreateSceneIndicesInfo.
pub struct UsdImagingCreateSceneIndicesInfo {
    /// Stage. Can be set after creation via StageSceneIndex::SetStage.
    pub stage: Option<Arc<Stage>>,
    /// Input args for StageSceneIndex (includeUnloadedPrims set automatically when displayUnloadedPrimsWithBounds).
    pub stage_scene_index_input_args: Option<HdContainerDataSourceHandle>,
    /// Add draw mode scene index in prototype propagation. C++ default: true.
    pub add_draw_mode_scene_index: bool,
    /// Display unloaded prims with bounds.
    pub display_unloaded_prims_with_bounds: bool,
    /// Optional callback to insert scene indices after the stage scene index.
    pub overrides_scene_index_callback: OverridesSceneIndexCallback,
}

impl Default for UsdImagingCreateSceneIndicesInfo {
    fn default() -> Self {
        Self {
            stage: None,
            stage_scene_index_input_args: None,
            add_draw_mode_scene_index: true,
            display_unloaded_prims_with_bounds: false,
            overrides_scene_index_callback: None,
        }
    }
}

/// Result of create_scene_indices.
///
/// Port of UsdImagingSceneIndices.
pub struct UsdImagingSceneIndices {
    /// The stage scene index (for SetStage/SetTime — no outer RwLock, uses interior mutability).
    pub stage_scene_index: Arc<StageSceneIndex>,
    /// Notice batching scene index after instancing (before proxy path translation).
    pub post_instancing_notice_batching_scene_index: HdSceneIndexHandle,
    /// Typed handle for flush() access.
    pub notice_batching_typed: Arc<RwLock<usd_hd::scene_index::HdNoticeBatchingSceneIndex>>,
    /// Selection scene index.
    pub selection_scene_index: HdSceneIndexHandle,
    /// The final scene index in the chain.
    pub final_scene_index: HdSceneIndexHandle,
}

/// Environment variable controlling when SetStage is called.
///
/// Port of USDIMAGING_SET_STAGE_AFTER_CHAINING_SCENE_INDICES (default: true).
fn should_set_stage_after_chaining_scene_indices() -> bool {
    // Default false: populate BEFORE chaining so notifications go to empty
    // observer list (fast). Render index initial traversal picks up all prims.
    // In C++ this is true but C++ scene index cascade is fast enough.
    // Our cascade has O(n*m) overhead that makes it impractical for >100 prims.
    tf_getenv_bool("USDIMAGING_SET_STAGE_AFTER_CHAINING_SCENE_INDICES", false)
}

/// Additional StageSceneIndex input args when displayUnloadedPrimsWithBounds.
/// Port of _AdditionalStageSceneIndexInputArgs.
fn additional_stage_scene_index_input_args(
    display_unloaded_prims_with_bounds: bool,
) -> Option<HdContainerDataSourceHandle> {
    if !display_unloaded_prims_with_bounds {
        return None;
    }
    let bool_ds = HdRetainedTypedSampledDataSource::new(true);
    Some(HdRetainedContainerDataSource::new_1(
        UsdImagingTokens::stage_scene_index_include_unloaded_prims().clone(),
        bool_ds,
    ))
}

/// Get stage name from root layer identifier for encapsulating display name.
/// Port of _GetStageName.
fn get_stage_name(stage: Option<&Arc<Stage>>) -> String {
    let Some(stage) = stage else {
        return String::new();
    };
    stage.get_root_layer().identifier().to_string()
}

/// Build instance data source names (materialBindings, purpose, geomModel, model, plugin names).
fn instance_data_source_names() -> Vec<Token> {
    let mut result = vec![
        crate::material_bindings_schema::MaterialBindingsSchema::get_schema_token(),
        (*HdPurposeSchema::get_schema_token()).clone(),
        crate::geom_model_schema::GeomModelSchema::get_schema_token(),
        crate::model_schema::ModelSchema::get_schema_token(),
    ];
    result.extend(UsdImagingSceneIndexPluginRegistry::instance_data_source_names());
    result
}

/// Build proxy path translation data source names (materialBindings, plugin names).
fn proxy_path_translation_data_source_names() -> Vec<Token> {
    let mut result =
        vec![crate::material_bindings_schema::MaterialBindingsSchema::get_schema_token()];
    result.extend(UsdImagingSceneIndexPluginRegistry::proxy_path_translation_data_source_names());
    result
}

/// Creates the full UsdImaging scene index chain.
///
/// Port of UsdImagingCreateSceneIndices.
/// Creates StageSceneIndex inside; stage can be set before or after chaining per env.
/// Order matches C++ sceneIndices.cpp:
/// Stage -> [overrides] -> [UnloadedDrawMode] -> ExtentResolving -> PiPrototypePropagating ->
/// [DrawMode] -> NiPrototypePropagating -> NoticeBatching -> InstanceProxyPathTranslation ->
/// MaterialBindingsResolving -> add_plugin_scene_indices -> Selection -> RenderSettingsFlattening
pub fn create_scene_indices(info: UsdImagingCreateSceneIndicesInfo) -> UsdImagingSceneIndices {
    let diag_scene_indices = std::env::var_os("USD_PROFILE_PREPARE").is_some();
    let diag = |msg: &str| {
        if diag_scene_indices {
            eprintln!("[scene_indices] {msg}");
        }
    };
    let stage_input_args = HdOverlayContainerDataSource::overlayed(
        additional_stage_scene_index_input_args(info.display_unloaded_prims_with_bounds),
        info.stage_scene_index_input_args.clone(),
    );
    let stage_ssi = StageSceneIndex::new_with_input_args(stage_input_args);
    // Wrap in delegate for the HdSceneIndexHandle chain.
    // stage_ssi has no outer RwLock (uses interior mutability), so use arc_scene_index_to_handle.
    let mut scene_index = arc_scene_index_to_handle(stage_ssi.clone());

    if !should_set_stage_after_chaining_scene_indices() {
        if let Some(ref stage) = info.stage {
            stage_ssi.set_stage(stage.clone());
        }
    }

    if let Some(ref cb) = info.overrides_scene_index_callback {
        scene_index = cb(scene_index);
    }

    if info.display_unloaded_prims_with_bounds {
        diag("unloaded_draw_mode");
        let unloaded = HdUnloadedDrawModeSceneIndex::new(Some(scene_index));
        scene_index = scene_index_to_handle(unloaded);
    }

    diag("extent_resolving");
    log::info!("[chain] creating extent_resolving");
    let extent_resolving = create_extent_resolving_scene_index(scene_index);
    scene_index = scene_index_to_handle(extent_resolving);

    diag("pi_propagating");
    log::info!("[chain] creating pi_propagating");
    let pi_propagating = PiPrototypePropagatingSceneIndex::new(scene_index);
    scene_index = scene_index_to_handle(pi_propagating);

    let instance_names = instance_data_source_names();
    let scene_index_append_callback: Option<
        Box<dyn Fn(HdSceneIndexHandle) -> HdSceneIndexHandle + Send + Sync>,
    > = if info.add_draw_mode_scene_index {
        Some(Box::new(|input| {
            let draw_mode = create_draw_mode_scene_index(input, None);
            scene_index_to_handle(draw_mode)
        }))
    } else {
        None
    };
    diag("ni_propagating");
    log::info!("[chain] creating ni_propagating");
    let ni_propagating = UsdImagingNiPrototypePropagatingSceneIndex::new_with_instance_names(
        scene_index,
        instance_names,
        scene_index_append_callback,
    );
    scene_index = scene_index_to_handle(ni_propagating);

    diag("notice_batching");
    log::info!("[chain] creating notice_batching");
    let notice_batching = HdNoticeBatchingSceneIndex::new(scene_index);
    log::info!("[chain] notice_batching done");
    log::info!("[chain] creating proxy_translation");
    let notice_batching_typed = notice_batching.clone();
    let post_instancing_notice_batching_scene_index = scene_index_to_handle(notice_batching);
    scene_index = post_instancing_notice_batching_scene_index.clone();

    diag("proxy_translation");
    log::info!("[chain] creating proxy_translation");
    let proxy_names = proxy_path_translation_data_source_names();
    let proxy_translation =
        InstanceProxyPathTranslationSceneIndex::new_with_proxy_path_names(scene_index, proxy_names);
    scene_index = scene_index_to_handle(proxy_translation);
    log::info!("[chain] proxy_translation done");

    diag("material_bindings");
    log::info!("[chain] creating material_bindings");
    let material_bindings = create_material_bindings_resolving_scene_index(scene_index);
    scene_index = scene_index_to_handle(material_bindings);
    log::info!("[chain] material_bindings done");

    diag("plugins");
    log::info!("[chain] add_plugin_scene_indices");
    scene_index = UsdImagingSceneIndexPluginRegistry::add_plugin_scene_indices(scene_index);
    log::info!("[chain] plugins done");

    diag("storm_filters");
    log::info!("[chain] append_storm_filters");
    scene_index = usd_hd_st::append_storm_filters(scene_index);
    log::info!("[chain] storm done");

    // Phase 2: flattening (xform, visibility, purpose, primvars, etc.)
    // C++ HdSt_FlatteningSceneIndexPlugin, insertionPhase=2.
    // Composes parent * local transforms, inherits visibility/purpose,
    // resolves material bindings and primvars from ancestors.
    {
        diag("flattening");
        let flattening_args = crate::flattened_data_source_providers
            ::usd_imaging_flattened_data_source_providers();
        let si = usd_hd::scene_index::HdFlatteningSceneIndex::new(
            Some(scene_index),
            Some(flattening_args),
        );
        scene_index = scene_index_to_handle(si);
    }
    log::info!("[chain] flattening done");

    diag("selection");
    log::info!("[chain] creating selection");
    let selection = SelectionSceneIndex::new(scene_index);
    let selection_scene_index = scene_index_to_handle(selection);
    scene_index = selection_scene_index.clone();

    diag("render_flattening");
    log::info!("[chain] creating render_flattening");
    let render_flattening = RenderSettingsFlatteningSceneIndex::new(scene_index);
    log::info!("[chain] render_flattening done");
    scene_index = scene_index_to_handle(render_flattening);

    if tf_getenv_bool("HD_USE_ENCAPSULATING_SCENE_INDICES", false) {
        scene_index = hd_make_encapsulating_scene_index(&[], scene_index);
        {
            let mut enc = scene_index.write();
            let name = format!("UsdImaging {}", get_stage_name(info.stage.as_ref()));
            enc.set_display_name(name);
        }
    }

    if should_set_stage_after_chaining_scene_indices() {
        if let Some(ref stage) = info.stage {
            diag("set_stage_after_chain");
            stage_ssi.set_stage(stage.clone());
        }
    }

    diag("done");

    UsdImagingSceneIndices {
        stage_scene_index: stage_ssi,
        post_instancing_notice_batching_scene_index,
        notice_batching_typed,
        selection_scene_index,
        final_scene_index: scene_index,
    }
}

/// Creates scene indices from input args schema.
///
/// Port of UsdImagingCreateSceneIndices(inputArgs, overridesSceneIndexCallback).
/// Extracts stage, stageSceneIndexInputArgs, addDrawModeSceneIndex, displayUnloadedPrimsWithBounds
/// from the schema and delegates to create_scene_indices(UsdImagingCreateSceneIndicesInfo).
pub fn create_scene_indices_from_input_args(
    input_args: HdContainerDataSourceHandle,
    overrides_scene_index_callback: OverridesSceneIndexCallback,
) -> UsdImagingSceneIndices {
    use super::usd_scene_index_input_args_schema::UsdSceneIndexInputArgsSchema;

    let schema = UsdSceneIndexInputArgsSchema::get_from_parent(&input_args);
    let schema = if schema.is_defined() {
        schema
    } else {
        UsdSceneIndexInputArgsSchema::from_container(Some(input_args.clone()))
    };
    let mut info = UsdImagingCreateSceneIndicesInfo::default();
    info.stage = schema.get_stage_typed_value();
    info.stage_scene_index_input_args = schema.get_container();
    info.overrides_scene_index_callback = overrides_scene_index_callback;
    info.add_draw_mode_scene_index = true; // C++ default

    if let Some(ds) = schema.get_add_draw_mode_scene_index() {
        info.add_draw_mode_scene_index = ds.get_typed_value(0.0);
    }
    if let Some(ds) = schema.get_display_unloaded_prims_with_bounds() {
        info.display_unloaded_prims_with_bounds = ds.get_typed_value(0.0);
    }

    create_scene_indices(info)
}

/// Apply UsdImaging scene index plugins to the given scene.
///
/// Port of _AddPluginSceneIndices from sceneIndices.cpp.
/// Call this when building the UsdImaging scene index chain, after
/// MaterialBindingsResolvingSceneIndex and before SelectionSceneIndex.
///
/// Plugins (e.g. ResolvingSceneIndexPlugin for UsdSkelImaging) are
/// auto-registered and applied in order.
///
/// # Example
///
/// ```ignore
/// let mut scene = stage_scene_index;
/// scene = material_bindings_resolving_scene_index;
/// scene = UsdImagingSceneIndices::add_plugin_scene_indices(scene);
/// scene = selection_scene_index;
/// ```
pub fn add_plugin_scene_indices(scene: HdSceneIndexHandle) -> HdSceneIndexHandle {
    UsdImagingSceneIndexPluginRegistry::add_plugin_scene_indices(scene)
}

/// Get instance data source names from all plugins.
///
/// Used when creating NiPrototypePropagatingSceneIndex to include
/// plugin-defined schemas (e.g. skelBinding) in instance aggregation.
pub fn instance_data_source_names_from_plugins() -> Vec<usd_tf::Token> {
    UsdImagingSceneIndexPluginRegistry::instance_data_source_names()
}

/// Get proxy path translation data source names from all plugins.
///
/// Used when creating InstanceProxyPathTranslationSceneIndex.
pub fn proxy_path_translation_data_source_names_from_plugins() -> Vec<usd_tf::Token> {
    UsdImagingSceneIndexPluginRegistry::proxy_path_translation_data_source_names()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skel::{
        BindingSchema, DataSourceResolvedPointsBasedPrim, SkeletonResolvingSceneIndex,
    };
    use std::path::PathBuf;
    use usd_hd::scene_index::si_ref;
    use usd_hd::schema::HdMeshSchema;

    fn open_reference_stage(relative_path: &str) -> Arc<Stage> {
        let fixture_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        Stage::open(
            fixture_path.to_str().expect("fixture path utf8"),
            usd_core::common::InitialLoadSet::LoadAll,
        )
        .expect("open reference stage")
    }

    fn has_mesh_topology(scene: &HdSceneIndexHandle, prim_path: &usd_sdf::Path) -> bool {
        let prim = si_ref(&scene).get_prim(prim_path);
        let Some(ds) = prim.data_source else {
            return false;
        };
        let mesh = HdMeshSchema::get_from_parent(&ds);
        let Some(topology) = mesh.get_topology() else {
            return false;
        };
        topology.get_face_vertex_counts().is_some() && topology.get_face_vertex_indices().is_some()
    }

    fn mesh_topology_sizes(
        scene: &HdSceneIndexHandle,
        prim_path: &usd_sdf::Path,
    ) -> Option<(usize, usize)> {
        let prim = si_ref(&scene).get_prim(prim_path);
        let ds = prim.data_source?;
        let mesh = HdMeshSchema::get_from_parent(&ds);
        let topology = mesh.get_topology()?;
        let counts = topology.get_face_vertex_counts()?.get_typed_value(0.0);
        let indices = topology.get_face_vertex_indices()?.get_typed_value(0.0);
        Some((counts.len(), indices.len()))
    }

    fn build_scene_through_plugins_unpopulated() -> (Arc<StageSceneIndex>, HdSceneIndexHandle) {
        let stage_ssi = StageSceneIndex::new_with_input_args(None);
        let mut scene_index = arc_scene_index_to_handle(stage_ssi.clone());

        let extent_resolving = create_extent_resolving_scene_index(scene_index);
        scene_index = scene_index_to_handle(extent_resolving);

        let pi_propagating = PiPrototypePropagatingSceneIndex::new(scene_index);
        scene_index = scene_index_to_handle(pi_propagating);

        let instance_names = instance_data_source_names();
        let scene_index_append_callback: Option<
            Box<dyn Fn(HdSceneIndexHandle) -> HdSceneIndexHandle + Send + Sync>,
        > = Some(Box::new(|input| {
            let draw_mode = create_draw_mode_scene_index(input, None);
            scene_index_to_handle(draw_mode)
        }));
        let ni_propagating = UsdImagingNiPrototypePropagatingSceneIndex::new_with_instance_names(
            scene_index,
            instance_names,
            scene_index_append_callback,
        );
        scene_index = scene_index_to_handle(ni_propagating);

        let notice_batching = HdNoticeBatchingSceneIndex::new(scene_index);
        scene_index = scene_index_to_handle(notice_batching);

        let proxy_translation = InstanceProxyPathTranslationSceneIndex::new_with_proxy_path_names(
            scene_index,
            proxy_path_translation_data_source_names(),
        );
        scene_index = scene_index_to_handle(proxy_translation);

        let material_bindings = create_material_bindings_resolving_scene_index(scene_index);
        scene_index = scene_index_to_handle(material_bindings);

        let plugin_scene = UsdImagingSceneIndexPluginRegistry::add_plugin_scene_indices(scene_index);

        (stage_ssi, plugin_scene)
    }

    fn build_scene_through_plugins(stage: Arc<Stage>) -> HdSceneIndexHandle {
        let (stage_ssi, plugin_scene) = build_scene_through_plugins_unpopulated();
        stage_ssi.set_stage(stage);
        plugin_scene
    }

    fn resolve_skeleton_type_after_chain(
        stage: Arc<Stage>,
        build_chain: impl FnOnce(HdSceneIndexHandle) -> HdSceneIndexHandle,
    ) -> String {
        let stage_ssi = StageSceneIndex::new_with_input_args(None);
        let scene = build_chain(arc_scene_index_to_handle(stage_ssi.clone()));
        let resolving = SkeletonResolvingSceneIndex::new(scene.clone());
        let resolving_handle = scene_index_to_handle(resolving);

        stage_ssi.set_stage(stage);

        let skeleton_path =
            usd_sdf::Path::from_string("/SkelChar/Skeleton").expect("skeleton path");
        si_ref(&resolving_handle)
            .get_prim(&skeleton_path)
            .prim_type
            .as_str()
            .to_string()
    }

    #[test]
    fn test_skeleton_resolution_survives_pre_plugin_chain_steps() {
        usd_core::schema_registry::register_builtin_schemas();

        let fixture =
            "testenv/testUsdImagingGLSkeleton/skeleton.usda";

        let stage_type = resolve_skeleton_type_after_chain(open_reference_stage(fixture), |scene| scene);
        assert_eq!(stage_type, "mesh");

        let extent_type = resolve_skeleton_type_after_chain(open_reference_stage(fixture), |scene| {
            let extent = create_extent_resolving_scene_index(scene);
            scene_index_to_handle(extent)
        });
        assert_eq!(extent_type, "mesh");

        let pi_type = resolve_skeleton_type_after_chain(open_reference_stage(fixture), |scene| {
            let extent = create_extent_resolving_scene_index(scene);
            let scene = scene_index_to_handle(extent);
            let pi = PiPrototypePropagatingSceneIndex::new(scene);
            scene_index_to_handle(pi)
        });
        assert_eq!(pi_type, "mesh");

        let ni_type = resolve_skeleton_type_after_chain(open_reference_stage(fixture), |scene| {
            let extent = create_extent_resolving_scene_index(scene);
            let scene = scene_index_to_handle(extent);
            let pi = PiPrototypePropagatingSceneIndex::new(scene);
            let scene = scene_index_to_handle(pi);
            let ni = UsdImagingNiPrototypePropagatingSceneIndex::new_with_instance_names(
                scene,
                instance_data_source_names(),
                None,
            );
            scene_index_to_handle(ni)
        });
        assert_eq!(ni_type, "mesh");

        let material_type = resolve_skeleton_type_after_chain(open_reference_stage(fixture), |scene| {
            let extent = create_extent_resolving_scene_index(scene);
            let scene = scene_index_to_handle(extent);
            let pi = PiPrototypePropagatingSceneIndex::new(scene);
            let scene = scene_index_to_handle(pi);
            let ni = UsdImagingNiPrototypePropagatingSceneIndex::new_with_instance_names(
                scene,
                instance_data_source_names(),
                None,
            );
            let scene = scene_index_to_handle(ni);
            let notice = HdNoticeBatchingSceneIndex::new(scene);
            let scene = scene_index_to_handle(notice);
            let proxy = InstanceProxyPathTranslationSceneIndex::new_with_proxy_path_names(
                scene,
                proxy_path_translation_data_source_names(),
            );
            let scene = scene_index_to_handle(proxy);
            let material = create_material_bindings_resolving_scene_index(scene);
            scene_index_to_handle(material)
        });
        assert_eq!(material_type, "mesh");
    }

    #[test]
    fn test_plugin_chain_resolves_skeleton_fixture_to_mesh_before_storm() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage(
            "testenv/testUsdImagingGLSkeleton/skeleton.usda",
        );
        let plugin_scene = build_scene_through_plugins(stage);
        let skeleton_path =
            usd_sdf::Path::from_string("/SkelChar/Skeleton").expect("skeleton path");
        let prim = plugin_scene
            .read()
            .get_prim(&skeleton_path);

        assert_eq!(prim.prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_create_scene_indices_resolves_skeleton_fixture_to_mesh() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage(
            "testenv/testUsdImagingGLSkeleton/skeleton.usda",
        );
        let scene_indices = create_scene_indices(UsdImagingCreateSceneIndicesInfo {
            stage: Some(stage),
            ..Default::default()
        });
        let skeleton_path =
            usd_sdf::Path::from_string("/SkelChar/Skeleton").expect("skeleton path");
        let prim = scene_indices
            .final_scene_index
            .read()
            .get_prim(&skeleton_path);

        assert_eq!(prim.prim_type.as_str(), "mesh");
        assert!(
            has_mesh_topology(&scene_indices.final_scene_index, &skeleton_path),
            "full create_scene_indices chain must preserve mesh topology for resolved skeleton guides",
        );
        let (face_count, index_count) = mesh_topology_sizes(
            &scene_indices.final_scene_index,
            &skeleton_path,
        )
        .expect("resolved skeleton guide topology sizes");
        assert!(
            face_count > 0 && index_count > 0,
            "resolved skeleton guide should expose non-empty bone mesh topology",
        );
    }

    #[test]
    fn test_create_scene_indices_inherits_skel_root_binding_for_skinned_mesh() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage(
            "testenv/testUsdImagingGLUsdSkel/arm.usda",
        );
        let scene_indices = create_scene_indices(UsdImagingCreateSceneIndicesInfo {
            stage: Some(stage),
            ..Default::default()
        });
        let mesh_path = usd_sdf::Path::from_string("/Model/Arm").expect("mesh path");
        let prim = scene_indices
            .final_scene_index
            .read()
            .get_prim(&mesh_path);
        let prim_source = prim.data_source.expect("mesh data source");
        let binding = BindingSchema::get_from_parent(&prim_source);
        let resolved = DataSourceResolvedPointsBasedPrim::new_from_scene(
            scene_indices.final_scene_index.clone(),
            &mesh_path,
            &prim_source,
        );

        assert!(binding.is_defined(), "skinned mesh must expose skelBinding");
        assert!(
            binding.get_has_skel_root(),
            "skinned mesh must inherit skelBinding.hasSkelRoot from its SkelRoot ancestor",
        );
        assert!(resolved.is_some(), "skinned mesh must resolve to a points-based skinning prim");
        assert_eq!(
            resolved
                .as_ref()
                .map(|resolved| resolved.get_skeleton_path().as_str()),
            Some("/Model/Skel"),
        );
    }
}
