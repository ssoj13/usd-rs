//! Hydra (Hd) - Rendering abstraction layer for USD.
//!
//! Hydra is USD's rendering architecture that provides:
//! - Abstract rendering interface for multiple backends
//! - Scene graph representation optimized for rendering
//! - Change tracking and incremental updates
//! - Instancing and aggregation support
//! - Extensible prim and task system
//!
//! # Architecture
//!
//! Hydra consists of several key components:
//!
//! - **Prims**: Scene objects (Rprims for geometry, Sprims for state, Bprims for buffers)
//! - **Scene Delegate**: Interface between scene data and render index
//! - **Render Index**: Central registry of all scene objects
//! - **Render Delegate**: Backend-specific rendering implementation
//! - **Tasks**: Rendering operations (e.g., draw, resolve)
//! - **Change Tracker**: Tracks dirty state for incremental updates
//! - **Data Sources**: Time-sampled, hierarchical scene data layer
//!
//! # Core Infrastructure
//!
//! This module currently provides foundational types:
//!
//! - **Types**: Core data types, dirty bits, tuple types, sampler parameters
//! - **Enums**: Interpolation modes, compare functions, cull styles
//! - **Tokens**: Standard identifiers for prims and properties
//! - **Data Sources**: Scene index data layer (containers, sampled values, locators)
//! - **Schemas**: Typed views into data source containers
//! - **Version**: API version tracking
//! - **Debug**: Debug codes and logging support
//! - **Perf Log**: Performance instrumentation (no-op macros in Rust)
//!
//! # Example
//!
//! ```rust
//! use usd_hd::*;
//!
//! // Create a tuple type for primvar data
//! let tuple_type = HdTupleType::new(HdType::FloatVec3, 100);
//! println!("Size: {} bytes", tuple_type.size_in_bytes());
//!
//! // Access standard tokens
//! println!("Points token: {}", tokens::POINTS.as_str());
//!
//! // Use sampler parameters
//! let sampler = HdSamplerParameters::default();
//! assert_eq!(sampler.wrap_s, HdWrap::Repeat);
//! ```
//!
//! # Future Modules
//!
//! Additional Hydra functionality will be added in future modules:
//! - `rprim` - Renderable primitives (meshes, curves, etc.)
//! - `sprim` - State primitives (cameras, lights, materials)
//! - `bprim` - Buffer primitives (render buffers)
//! - `scene_delegate` - Scene data interface
//! - `render_index` - Scene object registry
//! - `render_delegate` - Backend rendering interface
//! - `engine` - Rendering execution engine
//! - `task` - Rendering task system

pub mod aov;
pub mod basis_curves_topology;
pub mod change_tracker;
pub mod collection_expression_evaluator;
pub mod collection_predicate_library;
pub mod command;
pub mod data_source;
pub mod data_source_material_network_interface;
pub mod debug_codes;
pub mod dirty_bits_translator;
pub mod dirty_list;
pub mod draw_item;
pub mod drawing_coord;
pub mod enums;
pub mod ext_computation_context;
pub mod ext_computation_context_internal;
pub mod ext_computation_cpu_callback;
pub mod ext_computation_utils;
pub mod flat_normals;
pub mod flattened_data_source_provider;
pub mod flattened_data_source_providers;
pub mod flattened_overlay_data_source_provider;
pub mod flattened_primvars_data_source_provider;
pub mod flattened_purpose_data_source_provider;
pub mod flattened_visibility_data_source_provider;
pub mod flattened_xform_data_source_provider;
pub mod flo_debug;
pub mod geom_subset_struct;
pub mod instance_registry;
pub mod material_network;
pub mod material_network2_interface;
pub mod material_network_interface;
pub mod mesh_topology;
pub mod mesh_util;
pub mod perf_log;
pub mod plugin_render_delegate_unique_handle;
pub mod plugin_renderer_unique_handle;
pub mod prim;
pub mod prim_gather;
pub mod prim_type_index;
pub mod render;
pub mod render_delegate_adapter_renderer;
pub mod render_delegate_info;
pub mod render_index_adapter_scene_index;
pub mod render_pass_state;
pub mod render_thread;
pub mod renderer;
pub mod renderer_create_args;
pub mod renderer_plugin;
pub mod renderer_plugin_handle;
pub mod renderer_plugin_registry;
pub mod repr;
pub mod resource;
pub mod rprim_shared_data;
pub mod sampler_parameters;
pub mod scene_delegate;
pub mod scene_index;
pub mod scene_index_adapter_scene_delegate;
pub mod scene_index_util;
pub mod schema;
pub mod selection;
pub mod skinning_settings;
pub mod smooth_normals;
pub mod sorted_ids;
pub mod system_messages;
pub mod time_sample_array;
pub mod tokens;
pub mod topology;
pub mod types;
pub mod unit_test_delegate;
pub mod unit_test_helper;
pub mod unit_test_null_render_delegate;
pub mod unit_test_null_render_pass;
pub mod utils;
pub mod version;
pub mod vertex_adjacency;
pub mod vt_buffer_source;

// Re-export commonly used types
pub use utils::{
    RenderInstanceTracker, convert_hd_material_network_to_hd_material_schema,
    convert_vt_dictionary_to_container_ds, get_current_frame, has_active_render_pass_prim,
    has_active_render_settings_prim, print_scene_index, to_conform_window_policy,
};
pub use version::{HD_API_VERSION, HD_SHADER_API};

pub use enums::{
    HdBasisCurvesGeomStyle, HdBlendFactor, HdBlendOp, HdBorderColor, HdCompareFunction,
    HdCullStyle, HdDepthPriority, HdInterpolation, HdMagFilter, HdMeshGeomStyle, HdMinFilter,
    HdPointsGeomStyle, HdPolygonMode, HdStencilOp, HdWrap,
};

pub use change_tracker::{
    HdChangeTracker, HdRprimDirtyBits, HdTaskDirtyBits, dump_dirty_bits, stringify_dirty_bits,
};
pub use flat_normals::{
    MeshTopologyView, compute_flat_normals, compute_flat_normals_f64, compute_flat_normals_packed,
    compute_flat_normals_packed_f64,
};
pub use smooth_normals::{
    compute_smooth_normals, compute_smooth_normals_f64, compute_smooth_normals_packed,
    compute_smooth_normals_packed_f64,
};
pub use time_sample_array::{
    HdIndexedTimeSampleArray, HdTimeSampleArray, hd_get_contributing_sample_times_for_interval,
    hd_resample_neighbors, hd_resample_neighbors_value, hd_resample_raw_time_samples,
    hd_resample_raw_time_samples_indexed,
};
pub use types::{
    HD_FORMAT_COUNT, HD_TYPE_COUNT, HdDepthStencilType, HdDirtyBits, HdFormat, HdSamplerParameters,
    HdTupleType, HdType, HdVec4_2_10_10_10_Rev,
};
pub use vertex_adjacency::HdVertexAdjacency;

pub use sampler_parameters::{hd_get_sampler_params, hd_get_sampler_params_from_type};

pub use debug_codes::HdDebugCode;
pub use ext_computation_context::HdExtComputationContext;
pub use ext_computation_context_internal::HdExtComputationContextInternal;
pub use ext_computation_cpu_callback::{
    HdExtComputationCpuCallback, HdExtComputationCpuCallbackHandle,
    HdExtComputationCpuCallbackValue,
};
pub use skinning_settings::is_skinning_deferred;

// Re-export flattened data source provider types
pub use collection_expression_evaluator::HdCollectionExpressionEvaluator;
pub use collection_predicate_library::{
    HdCollectionPredicateLibrary, hd_get_collection_predicate_library,
};
pub use ext_computation_utils::{
    ComputationDependencyMap, ComputationDesc, SampledValueStore, build_dependency_map,
    get_computation_order, invoke_computations_cpu,
};
pub use flattened_data_source_provider::{
    HdFlattenedDataSourceProvider, HdFlattenedDataSourceProviderContext,
    HdFlattenedDataSourceProviderHandle, HdFlattenedDataSourceProviderVector,
};
pub use flattened_data_source_providers::hd_flattened_data_source_providers;
pub use flattened_overlay_data_source_provider::{
    HdFlattenedOverlayDataSourceProvider, make_flattened_data_source_providers,
};
pub use flattened_primvars_data_source_provider::HdFlattenedPrimvarsDataSourceProvider;

// Re-export data source types
pub use data_source::{
    HdBlockDataSource, HdBoolDataSourceHandle, HdContainerDataSource, HdContainerDataSourceHandle,
    HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceHashType, HdDataSourceLocator,
    HdDataSourceLocatorSet, HdLazyContainerDataSource, HdOverlayContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedSampledDataSource, HdRetainedSmallVectorDataSource,
    HdRetainedTypedMultisampledDataSource, HdRetainedTypedSampledDataSource, HdSampledDataSource,
    HdSampledDataSourceHandle, HdSampledDataSourceTime, HdTokenDataSourceHandle,
    HdTypedSampledDataSource, HdTypedSampledDataSourceHandle, HdValueExtract, HdVectorDataSource,
    HdVectorDataSourceHandle, SampledToTypedAdapter, cast_to_container, cast_to_vector,
    hd_container_get, hd_data_source_hash, hd_merge_contributing_sample_times,
};

// Re-export scene index types
pub use scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, HdCachingSceneIndex, HdEncapsulatingSceneIndex,
    HdFilteringSceneIndexBase, HdFlatteningSceneIndex, HdLegacyGeomSubsetSceneIndex,
    HdLegacyPrimSceneIndex, HdMergingSceneIndex, HdNoticeBatchingSceneIndex, HdRetainedSceneIndex,
    HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexObserver, HdSceneIndexPlugin,
    HdSceneIndexPluginRegistry, HdSceneIndexPrim, HdSceneIndexWeakHandle,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry,
    arc_scene_index_to_handle, si_ref, wire_filter_to_input,
};

// Re-export scene delegate types
pub use aov::{
    HdAovDescriptor, HdAovDescriptorList, HdAovSettingsMap, HdParsedAovToken,
    HdParsedAovTokenVector, HdRenderPassAovBinding, HdRenderPassAovBindingVector,
    hd_aov_has_depth_semantic, hd_aov_has_depth_stencil_semantic, hd_aov_tokens_make_lpe,
    hd_aov_tokens_make_primvar, hd_aov_tokens_make_shader,
};
pub use command::{
    HdCommandArgDescriptor, HdCommandArgs, HdCommandDescriptor, HdCommandDescriptors,
};
pub use scene_delegate::{
    HdDisplayStyle, HdExtComputationInputDescriptor, HdExtComputationInputDescriptorVector,
    HdExtComputationOutputDescriptor, HdExtComputationOutputDescriptorVector,
    HdExtComputationPrimvarDescriptor, HdExtComputationPrimvarDescriptorVector,
    HdIdVectorSharedPtr, HdInstancerContext, HdModelDrawMode, HdPrimvarDescriptor,
    HdPrimvarDescriptorVector, HdRenderBufferDescriptor, HdSyncRequestVector,
    HdVolumeFieldDescriptor, HdVolumeFieldDescriptorVector,
};

// Re-export prim types
pub use prim::{
    HdBasisCurves, HdBprim, HdCamera, HdInstancer, HdLight, HdLightType, HdMaterial, HdMesh,
    HdPoints, HdRenderBuffer, HdReprSelector, HdRprim, HdSceneDelegate, HdSprim,
};

// Re-export schema types
pub use schema::{
    HdBasisCurvesSchema, HdBasisCurvesTopologySchema, HdExtentSchema, HdGeomSubsetSchema,
    HdMeshSchema, HdMeshTopologySchema, HdPointsSchema, HdPrimvarsSchema, HdPurposeSchema,
    HdRendererCreateArgsSchema, HdSceneIndexInputArgsSchema, HdSchema, HdVisibilitySchema,
    HdXformSchema,
};

// Re-export render types
pub use render::{
    HdDriver, HdDriverVector, HdEngine, HdRenderDelegate, HdRenderDelegateSharedPtr, HdRenderIndex,
    HdRenderParam, HdRenderPass, HdRenderPassBase, HdRenderPassSharedPtr,
    HdRenderSettingDescriptor, HdResourceRegistrySharedPtr, HdRprimCollection, HdTask, HdTaskBase,
    HdTaskContext, HdTaskSharedPtr, HdTaskSharedPtrVector,
};

// Re-export draw/repr/topology types
pub use basis_curves_topology::HdBasisCurvesTopology;
pub use dirty_bits_translator::HdDirtyBitsTranslator;
pub use dirty_list::{HdDirtyList, HdDirtyListDataSource, HdReprSelectorVector};
pub use draw_item::{HdDrawItem, HdDrawItemTrait};
pub use drawing_coord::{
    HD_DRAWING_COORD_CUSTOM_SLOTS_BEGIN, HD_DRAWING_COORD_DEFAULT_NUM_SLOTS,
    HD_DRAWING_COORD_UNASSIGNED, HdDrawingCoord,
};
pub use geom_subset_struct::{HdGeomSubset, HdGeomSubsetType, HdGeomSubsets};
pub use instance_registry::{HdInstance, HdInstanceKey, HdInstanceRegistry};
pub use mesh_topology::HdMeshTopology;
pub use mesh_util::{
    HdMeshComputationResult, HdMeshEdgeIndexTable, HdMeshTriQuadBuilder, HdMeshUtil, HdQuadInfo,
};
pub use plugin_render_delegate_unique_handle::HdPluginRenderDelegateUniqueHandle;
pub use plugin_renderer_unique_handle::HdPluginRendererUniqueHandle;
pub use prim_gather::HdPrimGather;
pub use prim_type_index::HdPrimTypeIndex;
pub use render_delegate_adapter_renderer::HdRenderDelegateAdapterRenderer;
pub use render_delegate_info::HdRenderDelegateInfo;
pub use render_index_adapter_scene_index::HdRenderIndexAdapterSceneIndex;
pub use render_pass_state::HdRenderPassStateBase;
pub use render_thread::HdRenderThread;
pub use renderer::{HdLegacyRenderControlInterface, HdRenderer};
pub use renderer_create_args::HdRendererCreateArgs;
pub use renderer_plugin::{HdRendererPlugin, HdRendererPluginHandleType};
pub use renderer_plugin_handle::{HdRendererPluginHandle, HdRendererPluginTrait};
pub use renderer_plugin_registry::HdRendererPluginRegistry;
pub use repr::{HdDrawItemUniquePtr, HdDrawItemUniquePtrVector, HdRepr};
pub use rprim_shared_data::{HdRprimSharedData, TopologyToPrimvarVector};
pub use scene_index_adapter_scene_delegate::HdSceneIndexAdapterSceneDelegate;
pub use scene_index_util::{hd_make_encapsulating_scene_index, hd_use_encapsulating_scene_indices};
pub use selection::{
    HdPrimSelectionState, HdSelection, HdSelectionHighlightMode, HdSelectionSharedPtr,
};
pub use sorted_ids::HdSortedIds;
pub use system_messages::HdSystemMessageTokens;
pub use topology::{HdTopology, HdTopologyId};
pub use unit_test_null_render_pass::HdUnitTestNullRenderPass;
pub use vt_buffer_source::{
    HdVtBufferSource, hd_get_default_matrix_type, hd_get_value_data, hd_get_value_tuple_type,
};

// Re-export material network types
pub use material_network::{
    HdMaterialConnection2, HdMaterialDirtyBits, HdMaterialNetwork2, HdMaterialNetworkMap,
    HdMaterialNetworkV1, HdMaterialNode, HdMaterialNode2, HdMaterialRelationship,
    hd_convert_to_material_network2,
};
pub use material_network2_interface::HdMaterialNetwork2Interface;

// Re-export resource types
pub use resource::{
    HdBufferArray, HdBufferArrayHandle, HdBufferArrayRange, HdBufferArrayRangeContainer,
    HdBufferArrayRangeHandle, HdBufferArrayRangeWeakHandle, HdBufferArrayUsageHint,
    HdBufferArrayUsageHintBits, HdBufferArrayWeakHandle, HdBufferSource, HdBufferSourceHandle,
    HdBufferSourceState, HdBufferSourceWeakHandle, HdBufferSpec, HdBufferSpecVector, HdComputation,
    HdComputationHandle, HdComputedBufferSource, HdExtComputation, HdExtComputationDirtyBits,
    HdNullBufferSource, HdResolvedBufferSource, HdResourceRegistry, HdResourceRegistryHandle,
};

// Perf macros are automatically available via #[macro_export] -- no re-export needed.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Test version constants
        assert!(HD_API_VERSION > 0);
        assert!(HD_SHADER_API > 0);

        // Test enum values
        let _ = HdInterpolation::Vertex;
        let _ = HdCullStyle::Back;
        let _ = HdWrap::Repeat;

        // Test types
        let tuple = HdTupleType::new(HdType::Float, 1);
        assert_eq!(tuple.size_in_bytes(), 4);

        // Test tokens accessible
        assert_eq!(tokens::POINTS.as_str(), "points");
    }

    #[test]
    fn test_dirty_bits() {
        let dirty: HdDirtyBits = 0x01 | 0x02 | 0x04;
        assert_eq!(dirty, 0x07);

        let has_first = (dirty & 0x01) != 0;
        assert!(has_first);
    }

    #[test]
    fn test_sampler_parameters() {
        let default_params = HdSamplerParameters::default();
        // C++ defaults: wrapS=Repeat, wrapT=Repeat, wrapR=Clamp
        assert_eq!(default_params.wrap_s, HdWrap::Repeat);
        assert_eq!(default_params.wrap_t, HdWrap::Repeat);
        assert_eq!(default_params.wrap_r, HdWrap::Clamp);
        assert_eq!(default_params.min_filter, HdMinFilter::Nearest);
        assert_eq!(default_params.mag_filter, HdMagFilter::Nearest);

        let custom_params = HdSamplerParameters::new(
            HdWrap::Repeat,
            HdWrap::Repeat,
            HdWrap::Clamp,
            HdMinFilter::Linear,
            HdMagFilter::Nearest,
            HdBorderColor::OpaqueWhite,
            false,
            HdCompareFunction::Less,
            8,
        );

        assert_eq!(custom_params.wrap_s, HdWrap::Repeat);
        assert_eq!(custom_params.max_anisotropy, 8);
    }

    #[test]
    fn test_interpolation_modes() {
        use HdInterpolation::*;

        let modes = [Constant, Uniform, Varying, Vertex, FaceVarying, Instance];

        for mode in modes {
            assert!(!mode.as_str().is_empty());
        }
    }

    #[test]
    fn test_cull_style_operations() {
        assert_eq!(HdCullStyle::Back.invert(), HdCullStyle::Front);
        assert_eq!(HdCullStyle::Front.invert(), HdCullStyle::Back);

        let style = HdCullStyle::BackUnlessDoubleSided;
        assert_eq!(style.invert().invert(), style);
    }

    #[test]
    fn test_type_queries() {
        assert_eq!(HdType::FloatVec3.component_count(), 3);
        assert_eq!(HdType::FloatVec3.component_type(), HdType::Float);
        assert_eq!(HdType::FloatVec3.size_in_bytes(), 12);

        assert_eq!(HdType::FloatMat4.component_count(), 16);
        assert_eq!(HdType::FloatMat4.size_in_bytes(), 64);
    }

    #[test]
    fn test_packed_vector() {
        let vec = HdVec4_2_10_10_10_Rev::from_vec3(0.5, -0.5, 0.0);
        let (x, y, z) = vec.to_vec3();

        // Allow tolerance for fixed-point precision
        assert!((x - 0.5).abs() < 0.01);
        assert!((y + 0.5).abs() < 0.01);
        assert!(z.abs() < 0.01);
    }

    #[test]
    fn test_debug_codes() {
        let code = HdDebugCode::RprimAdded;
        assert_eq!(code.as_str(), "HD_RPRIM_ADDED");
        assert_eq!(code.log_target(), "hd::rprim");
    }

    #[test]
    fn test_perf_macros() {
        // Verify macros compile
        hd_trace_function!();
        hd_trace_scope!("test_scope");
        let cache_name = usd_tf::Token::new("cache");
        let cache_id = usd_sdf::Path::from("/id");
        hd_perf_cache_hit!(&cache_name, &cache_id);
        let counter_name = usd_tf::Token::new("counter");
        hd_perf_counter_incr!(&counter_name);
    }

    #[test]
    fn test_flat_normals() {
        use super::flat_normals::compute_flat_normals;
        use usd_px_osd::{MeshTopology, tokens};

        // Single quad: 0,1,2,3
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );
        let points: [[f32; 3]; 4] = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let normals = compute_flat_normals(&topology, &points);
        assert_eq!(normals.len(), 1);
        // Quad in XY plane → normal along +Z
        assert!((normals[0][2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_smooth_normals() {
        use super::smooth_normals::compute_smooth_normals;
        use super::vertex_adjacency::HdVertexAdjacency;
        use usd_px_osd::{MeshTopology, tokens};

        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![3, 3], // 2 triangles
            vec![0, 1, 2, 0, 2, 3],
        );
        let mut adj = HdVertexAdjacency::new();
        adj.build_adjacency_table(&topology);
        assert_eq!(adj.num_points(), 4);
        let points: [[f32; 3]; 4] = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let normals = compute_smooth_normals(&adj, 4, &points);
        assert_eq!(normals.len(), 4);
        // All vertices in XY plane → normals along +Z
        for n in &normals {
            assert!((n[2] - 1.0).abs() < 0.001, "normal {:?}", n);
        }
    }
}
