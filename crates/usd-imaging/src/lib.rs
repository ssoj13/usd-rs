//! UsdImaging - USD to Hydra scene index translation.
//!
//! Port of pxr/usdImaging. See `md/PARITY.md` for coverage details.
//!
//! This module provides the infrastructure for converting USD stage data into
//! Hydra scene indices for rendering. It implements the modern scene index
//! approach using data sources and adapters.
//!
//! The same crate also owns the application-facing rendering engine in
//! [`gl::Engine`]. That engine follows the reference HDX path:
//!
//! ```text
//! USD stage
//!  -> usd-imaging scene indices
//!  -> HdRenderIndex
//!  -> HdxTaskController
//!  -> HdEngine::execute()
//!  -> Storm geometry / AOV passes
//!  -> engine AOV bridge
//!  -> deferred post-FX replay
//! ```
//!
//! Deferred post-processing tasks (`aovInput`, `colorizeSelection`,
//! `colorCorrection`, `visualizeAov`, `present`) are emitted by `usd-hdx`
//! during `HdEngine::execute()` and replayed here after backend rendering, once
//! real AOV textures exist.
//!
//! # Architecture
//!
//! - [`StageSceneIndex`] - Main scene index that wraps a UsdStage
//! - [`PrimAdapter`] - Base trait for USD to Hydra conversion adapters
//! - [`AdapterRegistry`] - Registry for prim type to adapter mapping
//! - [`DataSourceStageGlobals`] - Stage-level context for data sources
//! - [`gl::Engine`] - Application-facing renderer that drives HDX + Storm
//!
//! # Key Concepts
//!
//! **Scene Index**: The modern Hydra approach where USD data is exposed through
//! a scene index that provides data sources. This replaces the older delegate
//! pattern.
//!
//! **Prim Adapters**: Each USD prim type (Mesh, Camera, etc.) has an adapter
//! that knows how to convert USD data to Hydra data sources.
//!
//! **Data Sources**: Lazy-evaluated data providers that read USD attributes
//! and convert them to Hydra-compatible formats.
//!
//! **Stage Globals**: Shared context passed to data sources containing stage
//! time, change tracking, and other global state.

pub mod scene_index_plugin;
pub mod scene_indices;

pub mod adapter_manager;
pub mod adapter_registry;
pub mod change_handler;
pub mod data_source_attribute;
pub mod data_source_prim;
pub mod data_source_stage_globals;
pub mod delegate;
pub mod index_proxy;
pub mod instancer_context;
pub mod prim_adapter;
pub mod stage_scene_index;
pub mod tokens;
pub mod types;
pub mod version;

// Data sources for specific prim types
pub mod data_source_attribute_color_space;
pub mod data_source_attribute_type_name;
pub mod data_source_basis_curves;
pub mod data_source_camera;
pub mod data_source_gprim;
pub mod data_source_hermite_curves;
pub mod data_source_implicits;
pub mod data_source_mapped;
pub mod data_source_material;
pub mod data_source_mesh;
pub mod data_source_nurbs_curves;
pub mod data_source_nurbs_patch;
pub mod data_source_point_instancer;
pub mod data_source_points;
pub mod data_source_primvars;
pub mod data_source_relationship;
pub mod data_source_render_prims;
pub mod data_source_stage;
pub mod data_source_tet_mesh;
pub mod data_source_usd_prim_info;
pub mod data_source_volume;

// Flattened data source providers
pub mod flattened_data_source_providers;
pub mod flattened_geom_model_data_source_provider;
pub mod flattened_material_bindings_data_source_provider;

// Hydra schemas
pub mod collection_material_binding_schema;
pub mod direct_material_binding_schema;
pub mod extents_hint_schema;
pub mod geom_model_schema;
pub mod material_binding_schema;
pub mod material_bindings_schema;
pub mod model_schema;
pub mod usd_prim_info_schema;
pub mod usd_render_product_schema;
pub mod usd_render_settings_schema;
pub mod usd_render_var_schema;
pub mod usd_scene_index_input_args_schema;
pub mod usd_stage_data_source;

// Scene indices
pub mod data_source_relocating_scene_index;
pub mod draw_mode_scene_index;
pub mod draw_mode_standin;
pub mod extent_resolving_scene_index;
pub mod instance_proxy_path_translation_scene_index;
pub mod material_bindings_resolving_scene_index;
pub mod ni_instance_aggregation_data_sources;
pub mod ni_instance_aggregation_impl;
pub mod ni_instance_aggregation_scene_index;
pub mod ni_instance_observer;
pub mod ni_prototype_propagating_scene_index;
pub mod ni_prototype_pruning_scene_index;
pub mod ni_prototype_scene_index;
pub mod pi_prototype_propagating_scene_index;
pub mod pi_prototype_scene_index;
pub mod render_settings_flattening_scene_index;
pub mod rerooting_container_data_source;
pub mod rerooting_scene_index;
pub mod root_overrides_scene_index;
pub mod selection_scene_index;
pub mod unloaded_draw_mode_scene_index;

// Prim adapters
pub mod api_schema_adapter;
pub mod camera_adapter;
pub mod collection_api_adapter;
pub mod coord_sys_adapter;
pub mod coord_sys_api_adapter;
pub mod curves_adapter;
pub mod draw_mode_adapter;
pub mod geom_model_api_adapter;
pub mod geom_subset_adapter;
pub mod geom_xform_vectors_schema;
pub mod gprim_adapter;
pub mod implicit_surface_adapter;
pub mod instance_adapter;
pub mod instancer_adapter;
pub mod light_adapter;
pub mod light_api_adapter;
pub mod material_adapter;
pub mod material_binding_api_adapter;
pub mod mesh_adapter;
pub mod points_adapter;
pub mod render_settings_adapter;
pub mod represented_by_ancestor_adapter;
pub mod scene_index_prim_adapter;
pub mod tet_mesh_adapter;
pub mod volume_adapter;

// Cache modules
pub mod collection_cache;
pub mod light_linking_cache;
pub mod primvar_desc_cache;
pub mod resolved_attribute_cache;

// Utility modules
pub mod implicit_surface_mesh_utils;
pub mod material_param_utils;
pub mod primvar_utils;
pub mod prototype_scene_index_utils;
pub mod texture_utils;

// Sub-modules (moved from top-level)
pub mod app_utils;
pub mod gl; // Rendering engine (backend-agnostic, GL/wgpu internals cfg-gated)
pub mod proc; // Procedural imaging (was usd_proc_imaging)
pub mod ri_pxr; // RenderMan imaging (was usd_ri_pxr_imaging)
pub mod skel; // Skeleton imaging (was usd_skel_imaging)
pub mod vol; // Volume imaging (was usd_vol_imaging) // Application utilities (was usd_app_utils)

// Re-exports
pub use adapter_manager::{
    AdapterEntry, AdapterManager, AdaptersEntry, ApiSchemaAdapter, ApiSchemaAdapterHandle,
};
pub use adapter_registry::AdapterRegistry;
pub use data_source_attribute_color_space::DataSourceAttributeColorSpace;
pub use data_source_attribute_type_name::DataSourceAttributeTypeName;
pub use data_source_basis_curves::{
    DataSourceBasisCurves, DataSourceBasisCurvesPrim, DataSourceBasisCurvesTopology,
};
pub use data_source_camera::{DataSourceCamera, DataSourceCameraPrim};
pub use data_source_gprim::DataSourceGprim;
pub use data_source_hermite_curves::{
    DataSourceHermiteCurves, DataSourceHermiteCurvesPrim, DataSourceHermiteCurvesTopology,
};
pub use data_source_implicits::{
    DataSourceImplicit, DataSourceImplicitsPrim, ImplicitGeometryType,
};
pub use data_source_mapped::{
    AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings, RelationshipMapping,
};
pub use data_source_material::{DataSourceMaterial, DataSourceMaterialPrim};
pub use data_source_mesh::{DataSourceMesh, DataSourceMeshPrim};
pub use data_source_nurbs_curves::{DataSourceNurbsCurves, DataSourceNurbsCurvesPrim};
pub use data_source_nurbs_patch::{DataSourceNurbsPatch, DataSourceNurbsPatchPrim};
pub use data_source_point_instancer::{
    DataSourcePointInstancerMask, DataSourcePointInstancerPrim, DataSourcePointInstancerTopology,
};
pub use data_source_points::DataSourcePointsPrim;
pub use data_source_prim::DataSourcePrim;
pub use data_source_primvars::{
    DataSourceCustomPrimvars, DataSourcePrimvar, DataSourcePrimvars, PrimvarMapping,
};
pub use data_source_relationship::DataSourceRelationship;
pub use data_source_render_prims::{
    DataSourceRenderPassPrim, DataSourceRenderProductPrim, DataSourceRenderSettingsPrim,
    DataSourceRenderVarPrim,
};
pub use data_source_stage::DataSourceStage;
pub use data_source_stage_globals::DataSourceStageGlobals;
pub use data_source_tet_mesh::{DataSourceTetMesh, DataSourceTetMeshPrim};
pub use data_source_usd_prim_info::DataSourceUsdPrimInfo;
pub use data_source_volume::{DataSourceVolumeFieldBindings, DataSourceVolumePrim};
pub use delegate::{CameraParams, LightParams, UsdImagingDelegate};
pub use index_proxy::IndexProxy;
pub use instancer_context::InstancerContext;
pub use prim_adapter::PrimAdapter;
pub use stage_scene_index::StageSceneIndex;
pub use tokens::UsdImagingTokens;
pub use types::{PopulationMode, PropertyInvalidationType};
pub use version::{VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};

// Hydra schema re-exports
pub use material_binding_schema::{MaterialBindingSchema, MaterialBindingSchemaBuilder};
pub use material_bindings_schema::MaterialBindingsSchema;
pub use model_schema::{ModelSchema, ModelSchemaBuilder};
pub use usd_prim_info_schema::{UsdPrimInfoSchema, UsdPrimInfoSchemaBuilder};
pub use usd_stage_data_source::UsdStageRefPtrDataSource;

// Flattened data source provider re-exports
pub use flattened_data_source_providers::{
    get_geom_model_provider, get_material_bindings_provider,
    usd_imaging_flattened_data_source_providers,
};
pub use flattened_geom_model_data_source_provider::FlattenedGeomModelDataSourceProvider;
pub use flattened_material_bindings_data_source_provider::FlattenedMaterialBindingsDataSourceProvider;

pub use scene_index_plugin::{
    UsdImagingSceneIndexPlugin, UsdImagingSceneIndexPluginHandle,
    UsdImagingSceneIndexPluginRegistry,
};
pub use scene_indices::{
    OverridesSceneIndexCallback, UsdImagingCreateSceneIndicesInfo, UsdImagingSceneIndices,
    add_plugin_scene_indices, create_scene_indices, create_scene_indices_from_input_args,
    instance_data_source_names_from_plugins, proxy_path_translation_data_source_names_from_plugins,
};

pub use api_schema_adapter::{APISchemaAdapter, NoOpAPISchemaAdapter};
pub use camera_adapter::CameraAdapter;
pub use collection_api_adapter::CollectionAPIAdapter;
pub use coord_sys_adapter::CoordSysAdapter;
pub use coord_sys_api_adapter::CoordSysAPIAdapter;
pub use curves_adapter::{
    BasisCurvesAdapter, HermiteCurvesAdapter, NurbsCurvesAdapter, NurbsPatchAdapter,
};
pub use draw_mode_adapter::DrawModeAdapter;
pub use geom_model_api_adapter::GeomModelAPIAdapter;
pub use geom_subset_adapter::GeomSubsetAdapter;
pub use gprim_adapter::GprimAdapter;
pub use implicit_surface_adapter::{
    CapsuleAdapter, ConeAdapter, CubeAdapter, CylinderAdapter as GeomCylinderAdapter, PlaneAdapter,
    SphereAdapter,
};
pub use instance_adapter::{InstanceAdapter, InstanceablePrimAdapter};
pub use instancer_adapter::PointInstancerAdapter;
pub use light_adapter::{
    CylinderLightAdapter, DiskLightAdapter, DistantLightAdapter, DomeLight1Adapter,
    DomeLightAdapter, GeometryLightAdapter, LightAdapter, LightFilterAdapter, PluginLightAdapter,
    PluginLightFilterAdapter, RectLightAdapter, SphereLightAdapter, is_scene_lights_enabled,
    set_scene_lights_enabled,
};
pub use light_api_adapter::LightAPIAdapter;
pub use material_adapter::{MaterialAdapter, NodeGraphAdapter, ShaderAdapter};
pub use material_binding_api_adapter::MaterialBindingAPIAdapter;
pub use mesh_adapter::MeshAdapter;
pub use points_adapter::PointsAdapter;
pub use render_settings_adapter::{
    RenderPassAdapter, RenderProductAdapter, RenderSettingsAdapter, RenderVarAdapter,
};
pub use represented_by_ancestor_adapter::RepresentedByAncestorPrimAdapter;
pub use scene_index_prim_adapter::SceneIndexPrimAdapter;
pub use tet_mesh_adapter::TetMeshAdapter;
pub use volume_adapter::{FieldAdapter, VolumeAdapter};

// Hydra schema re-exports
pub use collection_material_binding_schema::{
    CollectionMaterialBindingSchema, CollectionMaterialBindingSchemaBuilder,
};
pub use direct_material_binding_schema::{
    DirectMaterialBindingSchema, DirectMaterialBindingSchemaBuilder,
};
pub use extents_hint_schema::ExtentsHintSchema;
pub use geom_model_schema::{GeomModelSchema, GeomModelSchemaBuilder};
pub use geom_xform_vectors_schema::{GeomXformVectorsSchema, GeomXformVectorsSchemaBuilder};
pub use usd_render_product_schema::{UsdRenderProductSchema, UsdRenderProductSchemaBuilder};
pub use usd_render_settings_schema::{UsdRenderSettingsSchema, UsdRenderSettingsSchemaBuilder};
pub use usd_render_var_schema::{UsdRenderVarSchema, UsdRenderVarSchemaBuilder};
pub use usd_scene_index_input_args_schema::{
    UsdSceneIndexInputArgsSchema, UsdSceneIndexInputArgsSchemaBuilder,
};

// Scene index re-exports
pub use data_source_relocating_scene_index::UsdImagingDataSourceRelocatingSceneIndex;
pub use draw_mode_scene_index::{DrawModeSceneIndex, DrawModeSceneIndexHandle};
pub use draw_mode_standin::{
    BoundsStandin, CardsStandin, DrawModeStandin, DrawModeStandinHandle, OriginStandin,
};
pub use extent_resolving_scene_index::{
    ExtentResolvingSceneIndex, ExtentResolvingSceneIndexHandle, ResolvedExtentDataSource,
    create_extent_resolving_scene_index, create_extent_resolving_scene_index_with_args,
    extent_resolving_input_args,
};
pub use instance_proxy_path_translation_scene_index::{
    InstanceProxyPathTranslationSceneIndex, InstanceProxyPathTranslationSceneIndexHandle,
};
pub use material_bindings_resolving_scene_index::{
    MaterialBindingsResolvingSceneIndex, MaterialBindingsResolvingSceneIndexHandle,
    create_material_bindings_resolving_scene_index,
};
pub use ni_instance_aggregation_scene_index::{
    InstanceAggregationInfo, UsdImagingNiInstanceAggregationSceneIndex,
};
pub use ni_prototype_propagating_scene_index::{
    SceneIndexAppendCallback as NiSceneIndexAppendCallback,
    UsdImagingNiPrototypePropagatingSceneIndex,
};
pub use ni_prototype_pruning_scene_index::UsdImagingNiPrototypePruningSceneIndex;
pub use ni_prototype_scene_index::UsdImagingNiPrototypeSceneIndex;
pub use pi_prototype_propagating_scene_index::{
    PiPrototypePropagatingSceneIndex, PiPrototypePropagatingSceneIndexHandle,
};
pub use pi_prototype_scene_index::{PiPrototypeSceneIndex, PiPrototypeSceneIndexHandle};
pub use render_settings_flattening_scene_index::{
    RenderSettingsFlatteningSceneIndex, RenderSettingsFlatteningSceneIndexHandle,
    create_render_settings_flattening_scene_index,
};
pub use rerooting_container_data_source::UsdImagingRerootingContainerDataSource;
pub use rerooting_scene_index::HdRerootingSceneIndex;
pub use root_overrides_scene_index::HdRootOverridesSceneIndex;
pub use selection_scene_index::{SelectionSceneIndex, SelectionSceneIndexHandle};
pub use unloaded_draw_mode_scene_index::HdUnloadedDrawModeSceneIndex;

// Utility function re-exports
pub use primvar_utils::{
    is_valid_interpolation, usd_to_hd_interpolation, usd_to_hd_interpolation_token, usd_to_hd_role,
};
pub use prototype_scene_index_utils::{
    is_curve_type, is_implicit_surface, is_light_type, is_renderable_prim_type,
};

// Implicit surface mesh utilities re-exports
pub use implicit_surface_mesh_utils::{
    Axis, MeshTopology, gen_capsule_points, gen_cone_or_cylinder_xform, gen_plane_points,
    gen_sphere_or_cube_xform, get_capsule_topology, get_plane_topology, get_unit_cone_points,
    get_unit_cone_topology, get_unit_cube_points, get_unit_cube_topology, get_unit_cylinder_points,
    get_unit_cylinder_topology, get_unit_sphere_points, get_unit_sphere_topology,
};

// Material parameter utilities re-exports
pub use material_param_utils::{
    MaterialTerminal, ParamValue, build_material_terminals, extract_shader_params,
    get_texture_file_attr, is_texture_reader, resolve_asset_attr, resolve_asset_path,
};

// Texture utilities re-exports
pub use texture_utils::{
    FilterMode, TextureInfo, WrapMode, compute_udim_tile, expand_udim_tiles, extract_texture_info,
    extract_udim_pattern, get_filter_mode, get_wrap_mode, is_udim_pattern, resolve_texture_path,
};

// Cache re-exports
pub use collection_cache::CollectionCache;
pub use light_linking_cache::LightLinkingCache;
pub use primvar_desc_cache::{PrimvarDescCache, PrimvarDescriptor};
pub use resolved_attribute_cache::ResolvedAttributeCache;
