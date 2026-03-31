//! Material interface mapping schema for Hydra.
//!
//! Identifies a material node parameter using nodePath and inputName.
//! Corresponds to pxr/imaging/hd/materialInterfaceMappingSchema.h

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource,
};
use crate::schema::material_network::NODES;
use crate::schema::material_node::PARAMETERS as MATERIAL_NODE_PARAMETERS;
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Member token: "nodePath".
pub static NODE_PATH: Lazy<Token> = Lazy::new(|| Token::new("nodePath"));
/// Member token: "inputName".
pub static INPUT_NAME: Lazy<Token> = Lazy::new(|| Token::new("inputName"));

/// Data source for Token.
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token> + Send + Sync;
/// Handle to token data source.
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema identifying a material node parameter by nodePath and inputName.
///
/// Corresponds to C++ HdMaterialInterfaceMappingSchema.
#[derive(Debug, Clone)]
pub struct HdMaterialInterfaceMappingSchema {
    schema: HdSchema,
}

impl HdMaterialInterfaceMappingSchema {
    /// Create from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Returns the data source locator relative to the material network for the
    /// material node parameter indicated by the interface mapping.
    /// Ie. Returns locator: nodes/<nodePath>/parameters/<inputName>
    pub fn build_network_relative_locator(&self) -> HdDataSourceLocator {
        if let Some(node_path_ds) = self.get_node_path() {
            if let Some(input_name_ds) = self.get_input_name() {
                let node_path = node_path_ds.get_typed_value(0.0);
                let input_name = input_name_ds.get_typed_value(0.0);
                return HdDataSourceLocator::new(&[
                    NODES.clone(),
                    node_path,
                    MATERIAL_NODE_PARAMETERS.clone(),
                    input_name,
                ]);
            }
        }
        HdDataSourceLocator::empty()
    }

    /// Get node path token identifying the material node.
    pub fn get_node_path(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained::<Token>(&NODE_PATH)
    }

    /// Get input name token identifying the parameter on the node.
    pub fn get_input_name(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained::<Token>(&INPUT_NAME)
    }

    /// Build retained container with provided fields.
    pub fn build_retained(
        node_path: Option<HdTokenDataSourceHandle>,
        input_name: Option<HdTokenDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(np) = node_path {
            entries.push((NODE_PATH.clone(), np as HdDataSourceBaseHandle));
        }
        if let Some(inp) = input_name {
            entries.push((INPUT_NAME.clone(), inp as HdDataSourceBaseHandle));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdMaterialInterfaceMappingSchema.
#[derive(Default)]
pub struct HdMaterialInterfaceMappingSchemaBuilder {
    node_path: Option<HdTokenDataSourceHandle>,
    input_name: Option<HdTokenDataSourceHandle>,
}

impl HdMaterialInterfaceMappingSchemaBuilder {
    /// Create empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set node path.
    pub fn set_node_path(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.node_path = Some(v);
        self
    }

    /// Set input name.
    pub fn set_input_name(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.input_name = Some(v);
        self
    }

    /// Build container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdMaterialInterfaceMappingSchema::build_retained(self.node_path, self.input_name)
    }
}
