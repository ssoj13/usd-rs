
//! hdSt - Storm Render Delegate
//!
//! Storm is the default rasterization-based render delegate for Hydra.
//! It provides high-performance OpenGL/Vulkan/Metal rendering through
//! the Hgi (Hydra Graphics Interface) abstraction layer.
//!
//! # Features
//!
//! - **Multiple Prim Types**: Mesh, curves, points, volumes
//! - **Subdivision Surfaces**: OpenSubdiv integration
//! - **Instancing**: Efficient GPU instancing
//! - **Materials**: MaterialX and UsdPreviewSurface support
//! - **Advanced Lighting**: Point, directional, dome lights
//! - **Transparency**: Order-independent transparency
//! - **GPU Compute**: ExtComputations for procedural geometry
//!
//! # Architecture
//!
//! Storm follows the standard Hydra delegate pattern:
//!
//! ```text
//! HdRenderIndex
//!     |
//! HdStRenderDelegate
//!     +-> HdStRenderPass (executes rendering)
//!     +-> HdStResourceRegistry (manages GPU resources)
//!     +-> HdStMesh, HdStMaterial, etc. (prims)
//!     +-> HdStDrawBatch (optimized draw submission)
//! ```
//!
//! # Rendering Pipeline
//!
//! 1. **Sync Phase**: Prims update GPU buffers from scene data
//! 2. **Draw List Build**: RenderPass organizes prims into DrawBatches
//! 3. **PrepareDraw**: backend resources and batches are prepared
//! 4. **ExecuteDraw**: commands are recorded against the active AOV bindings
//! 5. **Submission**: work is submitted to the GPU via Hgi
//!
//! In the active `usd-rs` path, render-pass execution is driven by
//! `usd-imaging::gl::Engine` through HDX render-task state. That means Storm
//! must respect:
//!
//! - material-tag ordered geometry passes
//! - concrete AOV bindings for `color`, `depth`, `primId`, `instanceId`, and `elementId`
//! - the two-phase render pass structure used by the OpenUSD reference
//!
//! # Example
//!
//! ```ignore
//! use usd_hd_st::*;
//! use usd_hd::render::*;
//!
//! // Create Storm render delegate
//! let mut delegate = HdStRenderDelegate::new();
//!
//! // Create a mesh
//! let mesh_path = SdfPath::from_string("/mesh").unwrap();
//! let mesh = delegate.create_rprim(&Token::new("mesh"), mesh_path);
//!
//! // Create a render pass
//! let collection = HdRprimCollection::new(Token::new("geometry"));
//! let pass = delegate.create_render_pass(&index, &collection);
//!
//! // Sync and render
//! pass.sync();
//! pass.execute(&render_pass_state, &[]);
//! ```
//!
//! # GPU Backend Integration
//!
//! Storm uses Hgi for graphics API abstraction:
//!
//! - **HgiGL**: OpenGL 4.5+ backend
//! - **HgiMetal**: Metal backend (macOS/iOS)
//! - **HgiVulkan**: Vulkan backend
//!
//! The primary backend is **HgiWgpu** (wgpu/WebGPU), which provides full GPU
//! rendering. Legacy GL/Metal/Vulkan backends are scaffold-only.
//!
//! # Performance
//!
//! Storm is optimized for:
//! - Batched rendering (minimize draw calls)
//! - GPU instancing (millions of instances)
//! - Buffer sharing (reduce memory overhead)
//! - Shader compilation caching
//! - Frustum and occlusion culling
//!
//! # Current Focus
//!
//! Remaining work is concentrated in parity/completeness areas rather than the
//! basic render-pass skeleton:
//!
//! - remaining prim / shader parity gaps
//! - deeper AOV and post-processing coverage
//! - performance tuning and cache behavior
//! - scene-index filter parity

// Module declarations
pub mod asset_uv_texture_cpu_data;
pub mod basis_curves;
pub mod basis_curves_computations;
pub mod basis_curves_shader_key;
pub mod basis_curves_topology;
pub mod binding;
pub mod buffer_resource;
pub mod command_buffer;
pub mod computation;
pub mod culling_shader_key;
pub mod debug_codes;
pub mod draw_batch;
pub mod draw_item;
pub mod draw_item_instance;
pub mod draw_items_cache;
pub mod draw_program_key;
pub mod draw_target;
pub mod draw_target_render_pass_state;
pub mod ext_computation;
pub mod fallback_lighting_shader;
pub mod field;
#[cfg(feature = "gpu-culling")]
pub mod frustum_cull;
pub mod geometric_shader;
pub mod glsl_program;
pub mod glslfx_shader;
pub mod hgi_conversions;
pub mod instancer;
pub mod interleaved_memory_manager;
pub mod light;
pub mod lighting;
pub mod lighting_shader;
pub mod material;
pub mod material_network_shader;
#[cfg(feature = "mtlx")]
pub mod materialx_filter;
pub mod mesh;
pub mod mesh_shader_key;
pub mod mesh_topology;
pub mod package;
pub mod points;
pub mod points_shader_key;
pub mod prim_utils;
pub mod render_buffer;
pub mod render_delegate;
pub mod render_param;
pub mod render_pass;
pub mod render_pass_state;
pub mod resource_binder;
pub mod resource_registry;
pub mod shader_code;
pub mod shader_key;
pub mod shadow;
pub mod simple_lighting_shader;
pub mod staging_buffer;
pub mod subdivision;
pub mod texture_binder;
pub mod texture_cpu_data;
pub mod texture_handle;
pub mod texture_identifier;
pub mod texture_object;
pub mod tokens;
pub mod triangulate;
pub mod vbo_memory_manager;
pub mod volume;
pub mod volume_shader;
pub mod volume_shader_key;
pub mod wgsl_code_gen;

// Texture pipeline
pub mod dome_light_computations;
pub mod dynamic_cubemap_texture_impl;
pub mod dynamic_cubemap_texture_object;
pub mod dynamic_uv_texture_object;
pub mod field_subtexture_identifier;
pub mod hio_conversions;
pub mod ptex_mipmap_texture_loader;
pub mod sampler_object;
pub mod sampler_object_registry;
pub mod texture_handle_registry;
pub mod texture_object_registry;
pub mod texture_utils;

// Render buffer pool
pub mod render_buffer_pool;

// Vertex adjacency (Storm-side builder wrapping usd-hd core)
pub mod vertex_adjacency;

// ExtComp input/output pipeline
pub mod ext_comp_compute_shader;
pub mod ext_comp_computed_input_source;
pub mod ext_comp_cpu_computation;
pub mod ext_comp_gpu_computation;
pub mod ext_comp_gpu_computation_resource;
pub mod ext_comp_gpu_primvar_buffer;
pub mod ext_comp_input_source;
pub mod ext_comp_primvar_buffer_source;
pub mod ext_comp_scene_input_source;

// Buffer management
pub mod buffer_array_range;
pub mod dispatch_buffer;

// Scene index plugins
pub mod dependency_forwarding_scene_index;
pub mod flat_normals;
pub mod flattening_scene_index;
pub mod render_pass_shader_key;
pub mod scene_index_basis_curves_topology;
pub mod scene_index_draw_target;
pub mod scene_index_ext_comp;
pub mod scene_index_flat_normals;
pub mod scene_index_material_binding_resolving;
pub mod scene_index_material_override;
pub mod scene_index_mesh_topology;
pub mod scene_index_render_settings;
pub mod scene_index_smooth_normals;
pub mod scene_index_volume;

// Storm HDSI filter chain (applies all Storm-specific HDSI filters in C++ phase order)
pub mod storm_scene_index_filters;
pub use storm_scene_index_filters::append_storm_filters;

// Scene index plugins (plugin wrappers)
pub mod dependency_scene_index_plugin;
pub mod implicit_surface_scene_index_plugin;
pub mod material_primvar_transfer_scene_index_plugin;
pub mod node_identifier_resolving_scene_index_plugin;
pub mod nurbs_approximating_scene_index_plugin;
pub mod render_pass_prune_scene_index_plugin;
pub mod render_pass_visibility_scene_index_plugin;
pub mod tet_mesh_conversion_scene_index_plugin;
pub mod unbound_material_pruning_scene_index_plugin;
pub mod velocity_motion_resolving_scene_index_plugin;

// Re-exports for convenience
pub use basis_curves_shader_key::{BasisCurvesShaderKey, CurveDrawStyle, CurveNormalStyle};
pub use binding::{BindingRequest, BindingType};
pub use buffer_array_range::{
    HdStBufferArrayRangeContainer, HdStBufferArrayRangeSharedPtr, HdStBufferArrayRangeTrait,
};
pub use buffer_resource::{HdStBufferArrayRange, HdStBufferResource, HdStBufferResourceSharedPtr};
pub use command_buffer::HdStCommandBuffer;
pub use computation::{HdStComputation, HdStComputationDesc, HdStComputationSharedPtr};
pub use culling_shader_key::{CullingComputeShaderKey, CullingShaderKey};
pub use dispatch_buffer::{HdStDispatchBuffer, HdStDispatchBufferSharedPtr};
pub use draw_batch::{
    DrawBatch, DrawBatchSharedPtr, HdStDrawBatch, HdStDrawBatchSharedPtr, PipelineDrawBatch,
    ValidationResult,
};
pub use draw_item::{HdBufferArrayRange, HdStDrawItem, HdStDrawItemSharedPtr};
pub use ext_computation::{HdStExtComputation, HdStExtComputationSharedPtr};
pub use geometric_shader::{
    FvarPatchType, GsPrimitiveType, HdStGeometricShader, HdStGeometricShaderSharedPtr,
};
pub use glsl_program::{
    CompileStatus, HdStGLSLProgram, HdStGLSLProgramBuilder, HdStGLSLProgramSharedPtr,
};
pub use glslfx_shader::{HdStGlslfxShader, HdStGlslfxShaderSharedPtr};
pub use hgi_conversions::HdFormat;
pub use instancer::{HdStInstancePrimvar, HdStInstancer, HdStInstancerSharedPtr};
pub use interleaved_memory_manager::{
    InterleavedAllocation, InterleavedLayout, InterleavedMemoryManager,
    InterleavedMemoryManagerSharedPtr, VertexAttribute,
};
pub use light::{HdStLight, HdStLightSharedPtr, HdStShadowMap};
pub use lighting::{GpuLightType, LightGpuData, MAX_LIGHTS};
pub use lighting_shader::{
    HdStLightingShader, HdStLightingShaderSharedPtr, LightType, LightingModel, ShadowParams,
};
pub use material::{HdStMaterial, HdStMaterialSharedPtr};
pub use material_network_shader::{
    ExtractedMaterial, MaterialNetworkShader, MaterialNetworkShaderSharedPtr, ParamType,
    ShaderParam, TextureBindings,
};
pub use mesh::{HdStMesh, HdStMeshSharedPtr};
pub use mesh_shader_key::{MeshShaderKey, PrimvarInterp, ShadingModel};
pub use points_shader_key::PointsShaderKey;
pub use render_delegate::HdStRenderDelegate;
pub use render_param::HdStRenderParam;
pub use render_pass::{HdStRenderPass, HdStRenderPassSharedPtr};
pub use render_pass_state::{HdStPolygonRasterMode, HdStRenderPassState};
pub use resource_binder::ResourceBinder;
pub use resource_registry::{
    BufferArrayUsageHint, BufferSource, BufferSourceSharedPtr, BufferSpec, HdStResourceRegistry,
    HdStResourceRegistrySharedPtr, ManagedBar, ManagedBarSharedPtr,
};
pub use sampler_object::{
    HdStCubemapSamplerObject, HdStFieldSamplerObject, HdStPtexSamplerObject, HdStSamplerObject,
    HdStSamplerObjectSharedPtr, HdStSamplerObjectTrait, HdStUdimSamplerObject, HdStUvSamplerObject,
};
pub use shader_code::{
    HdStShaderCode, HdStShaderCodeSharedPtr, NamedTextureHandle, ResourceContext, ShaderParameter,
    ShaderStage, SimpleShaderCode, TextureSamplerParams,
};
pub use shader_key::{GeometricStyle, HdStShaderKey, PrimitiveType};
pub use staging_buffer::{StagingBuffer, StagingBufferSharedPtr, UploadOperation};
pub use texture_binder::{HdStTextureBinder, TextureBinderBuilder, TextureBinding};
pub use texture_cpu_data::HdStTextureCpuData;
pub use texture_handle::{
    HdStTextureHandle, HdStTextureHandleSharedPtr, MemoryRequest, SamplerParameters,
};
pub use texture_identifier::{HdStTextureIdentifier, SubtextureIdentifier};
pub use texture_object::{
    HdStTextureObject, HdStTextureObjectNamedList, HdStTextureObjectNamedPair,
    HdStTextureObjectSharedPtr, TextureType,
};
pub use texture_utils::{
    calc_mip_levels, calc_texture_memory, component_count, detect_texture_type,
    fit_dimensions_to_memory, format_byte_size, has_alpha, is_compressed_format,
};
pub use tokens::*; // Allow unused - tokens are referenced by name
pub use vbo_memory_manager::{VboAllocation, VboMemoryManager, VboMemoryManagerSharedPtr};
pub use volume_shader::{HdStVolumeShader, HdStVolumeShaderSharedPtr, VolumeFieldDescriptor};
pub use volume_shader_key::VolumeShaderKey;
pub use wgsl_code_gen::{MaterialParams, MaterialUniformData, WgslShaderCode};

// Auxiliary / utility re-exports
pub use debug_codes::{HdStDebugCode, is_debug_enabled};
pub use draw_item_instance::HdStDrawItemInstance;
pub use draw_items_cache::{HdDrawItemVecSharedPtr, HdStDrawItemsCache};
pub use draw_target_render_pass_state::{
    HdDepthPriority, HdRenderPassAovBinding, HdStDrawTargetRenderPassState as HdStDrawTargetRPS,
};
pub use fallback_lighting_shader::{
    HdStFallbackLightingShader, HdStFallbackLightingShaderSharedPtr,
};
pub use prim_utils::{
    HdBufferSpec, HdInterpolation, HdMeshGeomStyle, HdPrimvarDescriptor, compute_shared_primvar_id,
    is_shared_vertex_primvar_enabled, is_valid_bar, should_populate_constant_primvars,
};
pub use simple_lighting_shader::{HdStSimpleLightingShader, HdStSimpleLightingShaderSharedPtr};

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::render::HdRenderDelegate;
    use usd_sdf::Path as SdfPath;
    use usd_tf::Token;

    #[test]
    fn test_module_exports() {
        // Test that all main types are accessible
        let _delegate = HdStRenderDelegate::new();
        let _param = HdStRenderParam::new();
        let _state = HdStRenderPassState::new();
        let _registry = HdStResourceRegistry::new();
    }

    #[test]
    fn test_render_delegate_integration() {
        let mut delegate = HdStRenderDelegate::new();

        // Check supported types
        assert!(
            delegate
                .get_supported_rprim_types()
                .contains(&Token::new("mesh"))
        );

        // Create mesh
        let mesh_path = SdfPath::from_string("/mesh").unwrap();
        let mesh = delegate.create_rprim(&Token::new("mesh"), mesh_path);
        assert!(mesh.is_some());
    }
}
