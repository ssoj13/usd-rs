//! Hydra schema definitions.
//!
//! Schemas provide typed views into HdContainerDataSource objects, defining
//! the expected structure and fields for various scene data types.
//!
//! # Overview
//!
//! Schemas serve as structured accessors for scene data:
//! - Define expected fields and their types
//! - Provide builder patterns for construction
//! - Support sparse field sets (missing fields return None)
//! - Enable type-safe data access
//!
//! # Schema Categories
//!
//! ## Geometry Schemas
//! - [`HdPrimvarsSchema`] - Primitive variables container
//! - [`HdXformSchema`] - Transform matrix and stack control
//! - [`HdVisibilitySchema`] - Visibility state
//! - [`HdExtentSchema`] - Bounding box
//! - [`HdPurposeSchema`] - Render purpose classification
//! - [`HdMeshSchema`] - Mesh primitive
//! - [`HdMeshTopologySchema`] - Mesh connectivity
//! - [`HdBasisCurvesSchema`] - Curves primitive
//! - [`HdBasisCurvesTopologySchema`] - Curves topology
//! - [`HdNurbsCurvesSchema`] - NURBS curves primitive
//! - [`HdNurbsPatchSchema`] - NURBS patch surface
//! - [`HdPointsSchema`] - Point cloud primitive
//! - [`HdGeomSubsetSchema`] - Geometry face subsets
//! - [`HdSubdivisionTagsSchema`] - Subdivision surface tags (creases, corners)
//! - [`HdTetMeshSchema`] - Tetrahedral mesh primitive
//! - [`HdTetMeshTopologySchema`] - Tetrahedral mesh topology
//!
//! ## Volume Schemas
//! - [`HdVolumeFieldSchema`] - Volume field data
//! - [`HdVolumeFieldBindingSchema`] - Volume field bindings
//!
//! ## Implicit Geometry Schemas
//! - [`HdCapsuleSchema`] - Capsule primitive (cylinder with hemispherical caps)
//! - [`HdConeSchema`] - Cone primitive
//! - [`HdCubeSchema`] - Cube primitive
//! - [`HdCylinderSchema`] - Cylinder primitive
//! - [`HdSphereSchema`] - Sphere primitive
//! - [`HdPlaneSchema`] - Plane primitive
//!
//! ## Camera and Lighting Schemas
//! - [`HdCameraSchema`] - Camera parameters (projection, aperture, clipping)
//! - [`HdLightSchema`] - Light container
//! - [`HdCategoriesSchema`] - Light linking categories
//!
//! ## Material Schemas
//! - [`HdMaterialSchema`] - Material with render context networks
//! - [`HdMaterialNetworkSchema`] - Shader network definition
//! - [`HdMaterialNodeSchema`] - Single shader node
//!
//! ## Render Settings Schemas
//! - [`HdRenderSettingsSchema`] - Render resolution and output settings
//! - [`HdRenderProductSchema`] - Output product (AOV)
//! - [`HdRenderVarSchema`] - Render variable
//! - [`HdRenderBufferSchema`] - Render buffer configuration
//! - [`HdRenderPassSchema`] - Render pass settings
//!
//! ## Instancing Schemas
//! - [`HdInstanceSchema`] - Instance data
//! - [`HdInstanceCategoriesSchema`] - Instance categories
//! - [`HdInstanceIndicesSchema`] - Instance indices mapping
//! - [`HdInstancedBySchema`] - Instanced-by relationship
//! - [`HdInstancerTopologySchema`] - Instancer topology
//!
//! ## Selection and Collection Schemas
//! - [`HdSelectionSchema`] - Single selection with instance indices
//! - [`HdSelectionsSchema`] - Vector of selection entries
//! - [`HdCollectionSchema`] - Collection with membership expression
//! - [`HdCollectionsSchema`] - Named collections container
//!
//! ## Other Schemas
//! - [`HdCoordSysSchema`] - Coordinate system binding
//!
//! # Usage Pattern
//!
//! ```
//! use usd_hd::schema::*;
//! use usd_hd::data_source::*;
//!
//! // Retrieve schema from parent container
//! // let prim_container: HdContainerDataSourceHandle = ...;
//! // let xform = HdXformSchema::get_from_parent(&prim_container);
//! // if xform.is_defined() {
//! //     if let Some(matrix_ds) = xform.get_matrix() {
//! //         // Use matrix data source
//! //     }
//! // }
//! ```

mod base;
mod basis_curves;
mod basis_curves_topology;
mod builtin_material;
mod camera;
mod capsule;
mod categories;
mod collection;
mod collections;
mod cone;
pub mod container_schema;
mod coord_sys;
mod coord_sys_binding;
mod cube;
mod cylinder;
pub mod dependencies;
pub mod dependency;
mod display_filter;
mod ext_computation;
mod ext_computation_input_computation;
mod ext_computation_output;
mod ext_computation_primvar;
mod ext_computation_primvars;
mod extent;
mod geom_subset;
pub mod image_shader;
mod instance;
mod instance_categories;
mod instance_indices;
mod instanced_by;
mod instancer;
mod integrator;
mod legacy_display_style;
mod legacy_task_schema;
mod lens_distortion;
mod light;
pub mod material;
pub mod material_binding;
pub mod material_bindings;
mod material_connection;
mod material_interface_mapping_schema;
mod material_interface_parameter_schema;
pub mod material_interface_schema;
pub mod material_network;
pub mod material_node;
mod material_node_parameter;
pub mod material_override;
pub mod mesh;
pub mod mesh_topology;
mod nurbs_curves;
mod nurbs_patch;
mod nurbs_patch_trim_curve;
mod plane;
mod points;
mod prim_origin;
mod primvars;
mod purpose;
mod render_buffer;
mod render_capabilities;
mod render_pass;
mod render_product;
mod render_settings;
mod render_var;
mod renderer_create_args;
mod sample_filter;
mod scene_globals;
mod scene_index_input_args;
mod schema_type_defs;
mod selection;
mod selections;
mod sphere;
mod split_diopter;
mod subdivision_tags;
mod system;
mod tet_mesh;
mod tet_mesh_topology;
mod vector_schema;
pub mod visibility;
pub mod volume_field;
pub mod volume_field_binding;
pub mod xform;

pub use base::HdSchema;
pub use basis_curves::{HdBasisCurvesSchema, TOPOLOGY as BASIS_CURVES_TOPOLOGY};
pub use basis_curves_topology::{
    CURVE_INDICES, CURVE_VERTEX_COUNTS, HdBasisCurvesTopologySchema, WRAP,
};
pub use builtin_material::HdBuiltinMaterialSchema;
pub use camera::HdCameraSchema;
pub use capsule::HdCapsuleSchema;
pub use categories::HdCategoriesSchema;
pub use collection::HdCollectionSchema;
pub use collections::HdCollectionsSchema;
pub use cone::HdConeSchema;
pub use container_schema::{
    HdContainerOfSchemasSchema, HdContainerOfTypedSampledDataSourcesSchema, HdContainerSchema,
    SchemaFromContainer,
};
pub use coord_sys::{HdCoordSysSchema, HdCoordSysSchemaBuilder};
pub use coord_sys_binding::HdCoordSysBindingSchema;
pub use cube::HdCubeSchema;
pub use cylinder::HdCylinderSchema;
pub use dependencies::HdDependenciesSchema;
pub use dependency::{
    HdDependencySchema, HdDependencySchemaBuilder, HdLocatorDataSource, HdLocatorDataSourceHandle,
};
pub use display_filter::HdDisplayFilterSchema;
pub use ext_computation::{
    HdExtComputationSchema, HdExtComputationSchemaBuilder, HdSizetDataSource,
    HdSizetDataSourceHandle, HdStringDataSource, HdStringDataSourceHandle,
};
pub use ext_computation_input_computation::{
    HdExtComputationInputComputationSchema, HdExtComputationInputComputationSchemaBuilder,
    HdPathDataSource, HdPathDataSourceHandle,
};
pub use ext_computation_output::{
    HdExtComputationOutputSchema, HdExtComputationOutputSchemaBuilder, HdTupleTypeDataSource,
    HdTupleTypeDataSourceHandle,
};
pub use ext_computation_primvar::HdExtComputationPrimvarSchema;
pub use ext_computation_primvars::HdExtComputationPrimvarsSchema;
pub use extent::HdExtentSchema;
pub use geom_subset::{HdGeomSubsetSchema, INDICES as GEOM_SUBSET_INDICES, TYPE_POINT_SET};
pub use image_shader::HdImageShaderSchema;
pub use instance::HdInstanceSchema;
pub use instance_categories::HdInstanceCategoriesSchema;
pub use instance_indices::HdInstanceIndicesSchema;
pub use instanced_by::HdInstancedBySchema;
pub use instancer::HdInstancerTopologySchema;
pub use integrator::HdIntegratorSchema;
pub use legacy_display_style::{HdLegacyDisplayStyleSchema, HdLegacyDisplayStyleSchemaBuilder};
pub use legacy_task_schema::{
    HdLegacyTaskSchema, HdLegacyTaskSchemaBuilder, HdRprimCollectionDataSourceHandle,
    HdTokenVectorDataSourceHandle,
};
pub use lens_distortion::HdLensDistortionSchema;
pub use light::HdLightSchema;
pub use material::{HdMaterialSchema, MATERIAL};
pub use material_binding::{ALL_PURPOSE as MATERIAL_BINDING_ALL_PURPOSE, HdMaterialBindingSchema};
pub use material_bindings::{HdMaterialBindingsSchema, MATERIAL_BINDINGS};
pub use material_connection::HdMaterialConnectionSchema;
pub use material_interface_mapping_schema::{
    HdMaterialInterfaceMappingSchema, HdMaterialInterfaceMappingSchemaBuilder,
};
pub use material_interface_parameter_schema::{
    HdMaterialInterfaceParameterSchema, HdMaterialInterfaceParameterSchemaBuilder,
};
pub use material_interface_schema::{
    HdMaterialInterfaceParameterContainerSchema, HdMaterialInterfaceSchema,
    HdMaterialInterfaceSchemaBuilder, NestedTokenMap,
};
pub use material_network::HdMaterialNetworkSchema;
pub use material_node::HdMaterialNodeSchema;
pub use material_node_parameter::{
    HdMaterialNodeParameterContainerSchema, HdMaterialNodeParameterSchema,
};
pub use material_override::HdMaterialOverrideSchema;
pub use mesh::HdMeshSchema;
pub use mesh_topology::{
    HdMeshTopologySchema, HdTokenDataSourceHandle as HdMeshTopologyTokenDataSourceHandle,
};
pub use nurbs_curves::HdNurbsCurvesSchema;
pub use nurbs_patch::HdNurbsPatchSchema;
pub use nurbs_patch_trim_curve::HdNurbsPatchTrimCurveSchema;
pub use plane::HdPlaneSchema;
pub use points::HdPointsSchema;
pub use prim_origin::HdPrimOriginSchema;
pub use primvars::{
    CONSTANT as PRIMVAR_CONSTANT, FACE_VARYING as PRIMVAR_FACE_VARYING, HdPrimvarSchema,
    HdPrimvarSchemaBuilder, HdPrimvarsSchema, INDEXED_PRIMVAR_VALUE, INDICES as PRIMVAR_INDICES,
    INSTANCE as PRIMVAR_INSTANCE, PRIMVAR_VALUE, ROLE_COLOR, ROLE_EDGE_INDEX, ROLE_FACE_INDEX,
    ROLE_NORMAL, ROLE_POINT, ROLE_POINT_INDEX, ROLE_TEXTURE_COORDINATE, ROLE_VECTOR,
    TRANSFORM as PRIMVAR_TRANSFORM, UNIFORM as PRIMVAR_UNIFORM, VARYING as PRIMVAR_VARYING,
    VERTEX as PRIMVAR_VERTEX,
};
pub use purpose::HdPurposeSchema;
pub use render_buffer::HdRenderBufferSchema;
pub use render_capabilities::HdRenderCapabilitiesSchema;
pub use render_pass::HdRenderPassSchema;
pub use render_product::HdRenderProductSchema;
pub use render_settings::{
    ACTIVE as RENDER_SETTINGS_ACTIVE, HdRenderSettingsSchema,
    NAMESPACED_SETTINGS as RENDER_SETTINGS_NAMESPACED, RENDER_SETTINGS as RENDER_SETTINGS_TOKEN,
    SHUTTER_INTERVAL as RENDER_SETTINGS_SHUTTER_INTERVAL,
};
pub use render_var::HdRenderVarSchema;
pub use renderer_create_args::HdRendererCreateArgsSchema;
pub use sample_filter::HdSampleFilterSchema;
pub use scene_globals::HdSceneGlobalsSchema;
pub use scene_index_input_args::HdSceneIndexInputArgsSchema;
pub use schema_type_defs::*;
pub use selection::{HdSelectionSchema, HdSelectionSchemaBuilder};
pub use selections::HdSelectionsSchema;
pub use sphere::HdSphereSchema;
pub use split_diopter::HdSplitDiopterSchema;
pub use subdivision_tags::HdSubdivisionTagsSchema;
pub use system::{HdSystemSchema, HdSystemSchemaTokens, SYSTEM};
pub use tet_mesh::HdTetMeshSchema;
pub use tet_mesh_topology::HdTetMeshTopologySchema;
pub use vector_schema::{
    HdVectorOfSchemasSchema, HdVectorOfTypedSampledDataSourcesSchema, HdVectorSchema,
};
pub use visibility::HdVisibilitySchema;
pub use volume_field::HdVolumeFieldSchema;
pub use volume_field_binding::HdVolumeFieldBindingSchema;
pub use xform::{HdMatrixDataSourceHandle, HdXformSchema};
