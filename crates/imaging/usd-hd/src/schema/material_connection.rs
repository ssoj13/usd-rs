//! HdMaterialConnectionSchema - Defines upstream node and output for material connections.
//!
//! Corresponds to pxr/imaging/hd/materialConnectionSchema.h

use super::HdSchema;
use crate::data_source::HdContainerDataSourceHandle;
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

static UPSTREAM_NODE_PATH: Lazy<Token> = Lazy::new(|| Token::new("upstreamNodePath"));
static UPSTREAM_NODE_OUTPUT_NAME: Lazy<Token> = Lazy::new(|| Token::new("upstreamNodeOutputName"));

/// Schema for a material connection (upstream node path and output name).
#[derive(Debug, Clone)]
pub struct HdMaterialConnectionSchema {
    schema: HdSchema,
}

impl HdMaterialConnectionSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get upstream node path (token data source).
    pub fn get_upstream_node_path(
        &self,
    ) -> Option<Arc<dyn crate::data_source::HdTypedSampledDataSource<usd_tf::Token> + Send + Sync>>
    {
        self.schema.get_typed(&UPSTREAM_NODE_PATH)
    }

    /// Get upstream node output name (token data source).
    pub fn get_upstream_node_output_name(
        &self,
    ) -> Option<Arc<dyn crate::data_source::HdTypedSampledDataSource<usd_tf::Token> + Send + Sync>>
    {
        self.schema.get_typed(&UPSTREAM_NODE_OUTPUT_NAME)
    }

    /// Schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        static SCHEMA_TOKEN: Lazy<Token> = Lazy::new(|| Token::new("materialConnection"));
        &SCHEMA_TOKEN
    }
}
