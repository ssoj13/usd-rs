//! Render variable schema for Hydra.
//!
//! Defines a render variable (AOV - Arbitrary Output Variable) including
//! data type, source name, and source type.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

// Schema tokens
pub static RENDER_VAR: Lazy<Token> = Lazy::new(|| Token::new("renderVar"));
pub static PATH: Lazy<Token> = Lazy::new(|| Token::new("path"));
pub static DATA_TYPE: Lazy<Token> = Lazy::new(|| Token::new("dataType"));
pub static SOURCE_NAME: Lazy<Token> = Lazy::new(|| Token::new("sourceName"));
pub static SOURCE_TYPE: Lazy<Token> = Lazy::new(|| Token::new("sourceType"));
pub static NAMESPACED_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("namespacedSettings"));

// Typed data sources
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path>;
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;

pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema representing a render variable (AOV).
///
/// Provides access to:
/// - `path` - Output path for this AOV
/// - `dataType` - Data type (e.g., "color3f", "float")
/// - `sourceName` - Name of the source (e.g., "Ci", "a", "depth")
/// - `sourceType` - Type of source (e.g., "raw", "lpe")
/// - `namespacedSettings` - Renderer-specific settings
///
/// # Location
///
/// Default locator: `renderVar`
#[derive(Debug, Clone)]
pub struct HdRenderVarSchema {
    schema: HdSchema,
}

impl HdRenderVarSchema {
    /// Creates a new render variable schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves the schema from a parent container data source.
    ///
    /// Looks for a child container under the `renderVar` token.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&RENDER_VAR) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Checks if the schema is defined (has a valid container).
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Returns the output path for this AOV.
    pub fn get_path(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&PATH)
    }

    /// Returns the data type (e.g., "color3f", "float").
    pub fn get_data_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&DATA_TYPE)
    }

    /// Returns the source name (e.g., "Ci", "a", "depth").
    pub fn get_source_name(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&SOURCE_NAME)
    }

    /// Returns the source type (e.g., "raw", "lpe").
    pub fn get_source_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&SOURCE_TYPE)
    }

    /// Returns renderer-specific namespaced settings.
    pub fn get_namespaced_settings(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&NAMESPACED_SETTINGS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns the schema's identifying token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &RENDER_VAR
    }

    /// Returns the default data source locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_VAR.clone()])
    }

    /// Builds a retained container data source with the specified fields.
    ///
    /// This is a factory method that constructs a container with render variable data.
    pub fn build_retained(
        path: Option<HdPathDataSourceHandle>,
        data_type: Option<HdTokenDataSourceHandle>,
        source_name: Option<HdTokenDataSourceHandle>,
        source_type: Option<HdTokenDataSourceHandle>,
        namespaced_settings: Option<HdContainerDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = path {
            entries.push((PATH.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = data_type {
            entries.push((DATA_TYPE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = source_name {
            entries.push((SOURCE_NAME.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = source_type {
            entries.push((SOURCE_TYPE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = namespaced_settings {
            entries.push((NAMESPACED_SETTINGS.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdRenderVarSchema.
///
/// Provides a fluent interface for constructing render variable schemas
/// with sparse field assignment.
#[allow(dead_code)]
#[derive(Default)]
pub struct HdRenderVarSchemaBuilder {
    path: Option<HdPathDataSourceHandle>,
    data_type: Option<HdTokenDataSourceHandle>,
    source_name: Option<HdTokenDataSourceHandle>,
    source_type: Option<HdTokenDataSourceHandle>,
    namespaced_settings: Option<HdContainerDataSourceHandle>,
}

impl HdRenderVarSchemaBuilder {
    /// Creates a new builder.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the output path.
    #[allow(dead_code)]
    pub fn set_path(mut self, v: HdPathDataSourceHandle) -> Self {
        self.path = Some(v);
        self
    }

    /// Sets the data type.
    #[allow(dead_code)]
    pub fn set_data_type(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.data_type = Some(v);
        self
    }

    /// Sets the source name.
    #[allow(dead_code)]
    pub fn set_source_name(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.source_name = Some(v);
        self
    }

    /// Sets the source type.
    #[allow(dead_code)]
    pub fn set_source_type(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.source_type = Some(v);
        self
    }

    /// Sets renderer-specific namespaced settings.
    #[allow(dead_code)]
    pub fn set_namespaced_settings(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.namespaced_settings = Some(v);
        self
    }

    /// Builds and returns the configured container data source.
    #[allow(dead_code)]
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdRenderVarSchema::build_retained(
            self.path,
            self.data_type,
            self.source_name,
            self.source_type,
            self.namespaced_settings,
        )
    }
}
