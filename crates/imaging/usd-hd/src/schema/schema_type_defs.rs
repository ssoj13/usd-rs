//! Schema type definitions - type aliases for vector and container schemas.
//!
//! Corresponds to pxr/imaging/hd/schemaTypeDefs.h

use super::container_schema::{
    HdContainerOfSchemasSchema, HdContainerOfTypedSampledDataSourcesSchema, SchemaFromContainer,
};
use super::vector_schema::{HdVectorOfSchemasSchema, HdVectorOfTypedSampledDataSourcesSchema};
use crate::data_source::HdContainerDataSourceHandle;
use crate::schema::ext_computation_input_computation::HdExtComputationInputComputationSchema;
use crate::schema::ext_computation_output::HdExtComputationOutputSchema;
use crate::schema::instance_indices::HdInstanceIndicesSchema;
use crate::schema::material_connection::HdMaterialConnectionSchema;
use crate::schema::material_interface_mapping_schema::HdMaterialInterfaceMappingSchema;
use crate::schema::material_interface_parameter_schema::HdMaterialInterfaceParameterSchema;
use crate::schema::material_network::HdMaterialNetworkSchema;
use crate::schema::material_node::HdMaterialNodeSchema;
use crate::schema::material_node_parameter::HdMaterialNodeParameterSchema;
use crate::schema::render_product::HdRenderProductSchema;
use crate::schema::render_var::HdRenderVarSchema;

// =============================================================================
// Vectors of numeric types
// =============================================================================

/// Vector of HdIntArrayDataSource elements.
pub type HdIntArrayVectorSchema = HdVectorOfTypedSampledDataSourcesSchema<
    dyn crate::data_source::HdTypedSampledDataSource<usd_vt::Array<i32>> + Send + Sync,
>;

// =============================================================================
// Vectors of Schemas
// =============================================================================

/// Vector of render product schemas.
pub type HdRenderProductVectorSchema = HdVectorOfSchemasSchema<HdRenderProductSchema>;
/// Vector of render var schemas.
pub type HdRenderVarVectorSchema = HdVectorOfSchemasSchema<HdRenderVarSchema>;
/// Vector of instance indices schemas.
pub type HdInstanceIndicesVectorSchema = HdVectorOfSchemasSchema<HdInstanceIndicesSchema>;
/// Vector of material interface mapping schemas.
pub type HdMaterialInterfaceMappingVectorSchema =
    HdVectorOfSchemasSchema<HdMaterialInterfaceMappingSchema>;
/// Vector of material connection schemas.
pub type HdMaterialConnectionVectorSchema = HdVectorOfSchemasSchema<HdMaterialConnectionSchema>;

// =============================================================================
// Containers of sampled data sources
// =============================================================================

/// Container of sampled data sources (arbitrary names).
pub type HdSampledDataSourceContainerSchema = HdContainerOfTypedSampledDataSourcesSchema<
    dyn crate::data_source::HdSampledDataSource + Send + Sync,
>;

// =============================================================================
// Containers of schemas
// =============================================================================

/// Container of material node schemas.
pub type HdMaterialNodeContainerSchema = HdContainerOfSchemasSchema<HdMaterialNodeSchema>;
// NOTE: HdMaterialNodeParameterContainerSchema is defined in material_node_parameter.rs
/// Container of material network schemas.
pub type HdMaterialNetworkContainerSchema = HdContainerOfSchemasSchema<HdMaterialNetworkSchema>;
/// Container of material connection schemas.
pub type HdMaterialConnectionContainerSchema =
    HdContainerOfSchemasSchema<HdMaterialConnectionSchema>;
// NOTE: HdMaterialInterfaceParameterContainerSchema is defined in material_interface_schema.rs
/// Container of ext computation input computation schemas.
pub type HdExtComputationInputComputationContainerSchema =
    HdContainerOfSchemasSchema<HdExtComputationInputComputationSchema>;
/// Container of ext computation output schemas.
pub type HdExtComputationOutputContainerSchema =
    HdContainerOfSchemasSchema<HdExtComputationOutputSchema>;
/// Container of sampled data source container schemas.
pub type HdSampledDataSourceContainerContainerSchema =
    HdContainerOfSchemasSchema<HdSampledDataSourceContainerSchemaWrapper>;

// =============================================================================
// Containers of vectors of schemas
// =============================================================================

// Note: HdMaterialConnectionVectorContainerSchema would require container children to be vectors.
// C++ supports this via Schema::UnderlyingDataSource; we'd need SchemaFromVector trait.

// =============================================================================
// Schema wrappers (for schema types that need wrapper to fit container)
// =============================================================================

/// Wrapper to implement SchemaFromContainer for HdSampledDataSourceContainerSchema.
#[derive(Debug, Clone)]
pub struct HdSampledDataSourceContainerSchemaWrapper(
    #[allow(dead_code)] // Held for schema lifetime, accessed via container trait
    HdSampledDataSourceContainerSchema,
);

impl SchemaFromContainer for HdSampledDataSourceContainerSchemaWrapper {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self(HdSampledDataSourceContainerSchema::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        ))
    }
}

// =============================================================================
// SchemaFromContainer implementations
// =============================================================================

impl SchemaFromContainer for HdRenderProductSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdRenderVarSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdInstanceIndicesSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdMaterialInterfaceMappingSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdMaterialConnectionSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdMaterialNodeSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdMaterialNodeParameterSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdMaterialNetworkSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdMaterialInterfaceParameterSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdExtComputationInputComputationSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}

impl SchemaFromContainer for HdExtComputationOutputSchema {
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self::new(
            container
                .unwrap_or_else(|| crate::data_source::HdRetainedContainerDataSource::new_empty()),
        )
    }
}
