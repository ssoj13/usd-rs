//! HdImageShaderSchema - Image shader prim schema.
//!
//! Corresponds to pxr/imaging/hd/imageShaderSchema.h

use super::HdMaterialNetworkSchema;
use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

static IMAGE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("imageShader"));
static ENABLED: Lazy<Token> = Lazy::new(|| Token::new("enabled"));
static PRIORITY: Lazy<Token> = Lazy::new(|| Token::new("priority"));
static FILE_PATH: Lazy<Token> = Lazy::new(|| Token::new("filePath"));
static CONSTANTS: Lazy<Token> = Lazy::new(|| Token::new("constants"));
static MATERIAL_NETWORK: Lazy<Token> = Lazy::new(|| Token::new("materialNetwork"));

/// Schema for image shader prim (enabled, priority, filePath, constants, materialNetwork).
#[derive(Debug, Clone)]
pub struct HdImageShaderSchema {
    schema: HdSchema,
}

impl HdImageShaderSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container (looks up "imageShader" child).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&IMAGE_SHADER) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get enabled (bool).
    pub fn get_enabled(
        &self,
    ) -> Option<Arc<dyn crate::data_source::HdTypedSampledDataSource<bool> + Send + Sync>> {
        self.schema.get_typed(&ENABLED)
    }

    /// Get priority (int).
    pub fn get_priority(
        &self,
    ) -> Option<Arc<dyn crate::data_source::HdTypedSampledDataSource<i32> + Send + Sync>> {
        self.schema.get_typed(&PRIORITY)
    }

    /// Get file path (string).
    pub fn get_file_path(
        &self,
    ) -> Option<
        Arc<dyn crate::data_source::HdTypedSampledDataSource<std::string::String> + Send + Sync>,
    > {
        self.schema.get_typed(&FILE_PATH)
    }

    /// Get constants container (sampled data source container schema).
    pub fn get_constants(&self) -> Option<HdContainerDataSourceHandle> {
        let child = self.schema.get_container()?.get(&CONSTANTS)?;
        cast_to_container(&child)
    }

    /// Get material network schema.
    pub fn get_material_network(&self) -> HdMaterialNetworkSchema {
        if let Some(child) = self
            .schema
            .get_container()
            .and_then(|c| c.get(&MATERIAL_NETWORK))
        {
            if let Some(container) = cast_to_container(&child) {
                return HdMaterialNetworkSchema::new(container);
            }
        }
        HdMaterialNetworkSchema::new(crate::data_source::HdRetainedContainerDataSource::new_empty())
    }

    /// Schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &IMAGE_SHADER
    }

    /// Default locator for the imageShader container.
    pub fn get_default_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::new(&[IMAGE_SHADER.clone()])
    }

    /// Locator for enabled field.
    pub fn get_enabled_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::new(&[IMAGE_SHADER.clone(), ENABLED.clone()])
    }

    /// Locator for priority field.
    pub fn get_priority_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::new(&[IMAGE_SHADER.clone(), PRIORITY.clone()])
    }

    /// Locator for filePath field.
    pub fn get_file_path_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::new(&[IMAGE_SHADER.clone(), FILE_PATH.clone()])
    }

    /// Locator for constants container.
    pub fn get_constants_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::new(&[IMAGE_SHADER.clone(), CONSTANTS.clone()])
    }

    /// Locator for materialNetwork field.
    pub fn get_material_network_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::new(&[
            IMAGE_SHADER.clone(),
            MATERIAL_NETWORK.clone(),
        ])
    }
}
