//! Material interface parameter schema for Hydra.
//!
//! Describes a single interface parameter (public UI parameter) with mappings
//! to material node parameters.
//! Corresponds to pxr/imaging/hd/materialInterfaceParameterSchema.h

use super::HdSchema;
use super::material_interface_mapping_schema::HdMaterialInterfaceMappingSchema;
use super::vector_schema::HdVectorSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdVectorDataSourceHandle, cast_to_container, cast_to_vector,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Member token: "displayGroup".
pub static DISPLAY_GROUP: Lazy<Token> = Lazy::new(|| Token::new("displayGroup"));
/// Member token: "displayName".
pub static DISPLAY_NAME: Lazy<Token> = Lazy::new(|| Token::new("displayName"));
/// Member token: "mappings".
pub static MAPPINGS: Lazy<Token> = Lazy::new(|| Token::new("mappings"));

/// Data source for Token.
pub type HdTokenDataSource = dyn crate::data_source::HdTypedSampledDataSource<Token> + Send + Sync;
/// Handle to token data source.
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema for HdMaterialInterfaceMappingSchema vector (HdMaterialInterfaceMappingVectorSchema).
pub type HdMaterialInterfaceMappingVectorSchema = HdVectorSchema;

/// Schema describing a single interface parameter.
///
/// Corresponds to C++ HdMaterialInterfaceParameterSchema.
#[derive(Debug, Clone)]
pub struct HdMaterialInterfaceParameterSchema {
    schema: HdSchema,
}

impl HdMaterialInterfaceParameterSchema {
    /// Create from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get display group token.
    pub fn get_display_group(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained::<Token>(&DISPLAY_GROUP)
    }

    /// Get display name token.
    pub fn get_display_name(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained::<Token>(&DISPLAY_NAME)
    }

    /// Maps this interface parameter to a vector of target node parameters.
    pub fn get_mappings(&self) -> HdMaterialInterfaceMappingVectorSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(&MAPPINGS) {
                if let Some(vector) = cast_to_vector(&child) {
                    return HdMaterialInterfaceMappingVectorSchema::new(vector);
                }
            }
        }
        HdMaterialInterfaceMappingVectorSchema::empty()
    }

    /// Get mapping schema at index.
    pub fn get_mapping_element(&self, index: usize) -> HdMaterialInterfaceMappingSchema {
        let mappings = self.get_mappings();
        if let Some(elem) = mappings.get_element(index) {
            if let Some(container) = cast_to_container(&elem) {
                return HdMaterialInterfaceMappingSchema::new(container);
            }
        }
        HdMaterialInterfaceMappingSchema::new(
            crate::data_source::HdRetainedContainerDataSource::new_empty(),
        )
    }

    /// Build retained container with provided fields.
    pub fn build_retained(
        display_group: Option<HdTokenDataSourceHandle>,
        display_name: Option<HdTokenDataSourceHandle>,
        mappings: Option<HdVectorDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(dg) = display_group {
            entries.push((DISPLAY_GROUP.clone(), dg as HdDataSourceBaseHandle));
        }
        if let Some(dn) = display_name {
            entries.push((DISPLAY_NAME.clone(), dn as HdDataSourceBaseHandle));
        }
        if let Some(m) = mappings {
            entries.push((MAPPINGS.clone(), m as HdDataSourceBaseHandle));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdMaterialInterfaceParameterSchema.
#[derive(Default)]
pub struct HdMaterialInterfaceParameterSchemaBuilder {
    display_group: Option<HdTokenDataSourceHandle>,
    display_name: Option<HdTokenDataSourceHandle>,
    mappings: Option<HdVectorDataSourceHandle>,
}

impl HdMaterialInterfaceParameterSchemaBuilder {
    /// Create empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set display group.
    pub fn set_display_group(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.display_group = Some(v);
        self
    }

    /// Set display name.
    pub fn set_display_name(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.display_name = Some(v);
        self
    }

    /// Set mappings vector.
    pub fn set_mappings(mut self, v: HdVectorDataSourceHandle) -> Self {
        self.mappings = Some(v);
        self
    }

    /// Build container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdMaterialInterfaceParameterSchema::build_retained(
            self.display_group,
            self.display_name,
            self.mappings,
        )
    }
}
