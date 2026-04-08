//! HdMaterialNodeParameterSchema - Value data source for a material node parameter.
//!
//! Corresponds to pxr/imaging/hd/materialNodeParameterSchema.h

use super::HdSchema;
use crate::data_source::HdContainerDataSourceHandle;
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

static VALUE: Lazy<Token> = Lazy::new(|| Token::new("value"));
static COLOR_SPACE: Lazy<Token> = Lazy::new(|| Token::new("colorSpace"));
static TYPE_NAME: Lazy<Token> = Lazy::new(|| Token::new("typeName"));

/// Schema for a material node parameter (value, colorSpace, typeName).
#[derive(Debug, Clone)]
pub struct HdMaterialNodeParameterSchema {
    schema: HdSchema,
}

impl HdMaterialNodeParameterSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get value (sampled data source).
    /// Returns the child data source for "value" - caller may use as_sampled() to use as sampled.
    pub fn get_value(&self) -> Option<crate::data_source::HdDataSourceBaseHandle> {
        let container = self.schema.get_container()?;
        container.get(&VALUE)
    }

    /// Get color space (token data source).
    pub fn get_color_space(
        &self,
    ) -> Option<Arc<dyn crate::data_source::HdTypedSampledDataSource<usd_tf::Token> + Send + Sync>>
    {
        self.schema.get_typed(&COLOR_SPACE)
    }

    /// Get type name (token data source).
    pub fn get_type_name(
        &self,
    ) -> Option<Arc<dyn crate::data_source::HdTypedSampledDataSource<usd_tf::Token> + Send + Sync>>
    {
        self.schema.get_typed(&TYPE_NAME)
    }

    /// Schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        static SCHEMA_TOKEN: Lazy<Token> = Lazy::new(|| Token::new("materialNodeParameter"));
        &SCHEMA_TOKEN
    }
}

/// Container of MaterialNodeParameter schemas (keyed by parameter name).
/// Used by material override parameter values.
pub type HdMaterialNodeParameterContainerSchema = super::container_schema::HdContainerSchema;
