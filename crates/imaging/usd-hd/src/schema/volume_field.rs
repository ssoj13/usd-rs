#![allow(dead_code)]
//! Volume field schema for Hydra.
//!
//! Defines volume field data including file path, field name/index,
//! data type, and vector data role hint.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_sdf::AssetPath;
use usd_tf::Token;

/// Volume field schema token
pub static VOLUME_FIELD: LazyLock<Token> = LazyLock::new(|| Token::new("volumeField"));
/// File path token
pub static FILE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("filePath"));
/// Field name token
pub static FIELD_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("fieldName"));
/// Field index token
pub static FIELD_INDEX: LazyLock<Token> = LazyLock::new(|| Token::new("fieldIndex"));
/// Field data type token
pub static FIELD_DATA_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("fieldDataType"));
/// Vector data role hint token
pub static VECTOR_DATA_ROLE_HINT: LazyLock<Token> =
    LazyLock::new(|| Token::new("vectorDataRoleHint"));

/// Data source for AssetPath values
pub type HdAssetPathDataSource = dyn HdTypedSampledDataSource<AssetPath>;
/// Arc handle to AssetPath data source
pub type HdAssetPathDataSourceHandle = Arc<HdAssetPathDataSource>;
/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;
/// Data source for i32 values
pub type HdIntDataSource = dyn HdTypedSampledDataSource<i32>;
/// Arc handle to i32 data source
pub type HdIntDataSourceHandle = Arc<HdIntDataSource>;

/// Schema representing volume field data.
///
/// Provides access to:
/// - `filePath` - Path to volume field file
/// - `fieldName` - Name of the field
/// - `fieldIndex` - Index of the field
/// - `fieldDataType` - Data type of the field
/// - `vectorDataRoleHint` - Hint for vector data role
///
/// # Location
///
/// Default locator: `volumeField`
#[derive(Debug, Clone)]
pub struct HdVolumeFieldSchema {
    schema: HdSchema,
}

impl HdVolumeFieldSchema {
    /// Constructs a volume field schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves volume field schema from parent container at "volumeField" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&VOLUME_FIELD) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema is non-empty.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Gets the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets file path to volume field file.
    pub fn get_file_path(&self) -> Option<HdAssetPathDataSourceHandle> {
        self.schema.get_typed(&FILE_PATH)
    }

    /// Gets field name.
    pub fn get_field_name(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&FIELD_NAME)
    }

    /// Gets field index.
    pub fn get_field_index(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&FIELD_INDEX)
    }

    /// Gets field data type.
    pub fn get_field_data_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&FIELD_DATA_TYPE)
    }

    /// Gets vector data role hint.
    pub fn get_vector_data_role_hint(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&VECTOR_DATA_ROLE_HINT)
    }

    /// Returns the schema token for volume field.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &VOLUME_FIELD
    }

    /// Returns the default locator for volume field schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VOLUME_FIELD.clone()])
    }

    /// Returns the locator for file path.
    pub fn get_file_path_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VOLUME_FIELD.clone(), FILE_PATH.clone()])
    }

    /// Returns the locator for field name.
    pub fn get_field_name_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VOLUME_FIELD.clone(), FIELD_NAME.clone()])
    }

    /// Returns the locator for field index.
    pub fn get_field_index_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VOLUME_FIELD.clone(), FIELD_INDEX.clone()])
    }

    /// Returns the locator for field data type.
    pub fn get_field_data_type_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VOLUME_FIELD.clone(), FIELD_DATA_TYPE.clone()])
    }

    /// Returns the locator for vector data role hint.
    pub fn get_vector_data_role_hint_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VOLUME_FIELD.clone(), VECTOR_DATA_ROLE_HINT.clone()])
    }

    /// Builds a retained container with volume field parameters.
    ///
    /// # Parameters
    /// All volume field settings as optional data source handles.
    pub fn build_retained(
        file_path: Option<HdAssetPathDataSourceHandle>,
        field_name: Option<HdTokenDataSourceHandle>,
        field_index: Option<HdIntDataSourceHandle>,
        field_data_type: Option<HdTokenDataSourceHandle>,
        vector_data_role_hint: Option<HdTokenDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries = Vec::new();

        if let Some(fp) = file_path {
            entries.push((FILE_PATH.clone(), fp as HdDataSourceBaseHandle));
        }
        if let Some(fn_) = field_name {
            entries.push((FIELD_NAME.clone(), fn_ as HdDataSourceBaseHandle));
        }
        if let Some(fi) = field_index {
            entries.push((FIELD_INDEX.clone(), fi as HdDataSourceBaseHandle));
        }
        if let Some(fdt) = field_data_type {
            entries.push((FIELD_DATA_TYPE.clone(), fdt as HdDataSourceBaseHandle));
        }
        if let Some(vdrh) = vector_data_role_hint {
            entries.push((
                VECTOR_DATA_ROLE_HINT.clone(),
                vdrh as HdDataSourceBaseHandle,
            ));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdVolumeFieldSchema.
///
/// Provides a fluent interface for constructing volume field schemas.
pub struct HdVolumeFieldSchemaBuilder {
    file_path: Option<HdAssetPathDataSourceHandle>,
    field_name: Option<HdTokenDataSourceHandle>,
    field_index: Option<HdIntDataSourceHandle>,
    field_data_type: Option<HdTokenDataSourceHandle>,
    vector_data_role_hint: Option<HdTokenDataSourceHandle>,
}

impl HdVolumeFieldSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            file_path: None,
            field_name: None,
            field_index: None,
            field_data_type: None,
            vector_data_role_hint: None,
        }
    }

    /// Sets the file path.
    pub fn set_file_path(mut self, file_path: HdAssetPathDataSourceHandle) -> Self {
        self.file_path = Some(file_path);
        self
    }

    /// Sets the field name.
    pub fn set_field_name(mut self, field_name: HdTokenDataSourceHandle) -> Self {
        self.field_name = Some(field_name);
        self
    }

    /// Sets the field index.
    pub fn set_field_index(mut self, field_index: HdIntDataSourceHandle) -> Self {
        self.field_index = Some(field_index);
        self
    }

    /// Sets the field data type.
    pub fn set_field_data_type(mut self, field_data_type: HdTokenDataSourceHandle) -> Self {
        self.field_data_type = Some(field_data_type);
        self
    }

    /// Sets the vector data role hint.
    pub fn set_vector_data_role_hint(
        mut self,
        vector_data_role_hint: HdTokenDataSourceHandle,
    ) -> Self {
        self.vector_data_role_hint = Some(vector_data_role_hint);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdVolumeFieldSchema::build_retained(
            self.file_path,
            self.field_name,
            self.field_index,
            self.field_data_type,
            self.vector_data_role_hint,
        )
    }
}

impl Default for HdVolumeFieldSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_field_schema_empty() {
        let empty_container: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[]);
        let schema = HdVolumeFieldSchema::get_from_parent(&empty_container);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_volume_field_schema_tokens() {
        assert_eq!(VOLUME_FIELD.as_str(), "volumeField");
        assert_eq!(FILE_PATH.as_str(), "filePath");
        assert_eq!(FIELD_NAME.as_str(), "fieldName");
        assert_eq!(FIELD_INDEX.as_str(), "fieldIndex");
        assert_eq!(FIELD_DATA_TYPE.as_str(), "fieldDataType");
        assert_eq!(VECTOR_DATA_ROLE_HINT.as_str(), "vectorDataRoleHint");
    }

    #[test]
    fn test_volume_field_schema_locators() {
        let default_loc = HdVolumeFieldSchema::get_default_locator();
        assert_eq!(default_loc.elements().len(), 1);

        let file_path_loc = HdVolumeFieldSchema::get_file_path_locator();
        assert_eq!(file_path_loc.elements().len(), 2);
    }
}
