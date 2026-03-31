//! Legacy task schema for Hydra.
//!
//! Specifies a Hydra task by providing a task factory and data.
//! Corresponds to pxr/imaging/hd/legacyTaskSchema.h

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdLegacyTaskFactoryHandle, HdSampledDataSourceHandle, HdTypedSampledDataSource,
    cast_to_container,
};
use crate::render::HdRprimCollection;
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Schema token: "task".
pub static TASK: Lazy<Token> = Lazy::new(|| Token::new("task"));
/// Member token: "factory".
pub static FACTORY: Lazy<Token> = Lazy::new(|| Token::new("factory"));
/// Member token: "parameters".
pub static PARAMETERS: Lazy<Token> = Lazy::new(|| Token::new("parameters"));
/// Member token: "collection".
pub static COLLECTION: Lazy<Token> = Lazy::new(|| Token::new("collection"));
/// Member token: "renderTags".
pub static RENDER_TAGS: Lazy<Token> = Lazy::new(|| Token::new("renderTags"));

/// Data source for HdRprimCollection.
pub type HdRprimCollectionDataSource = dyn HdTypedSampledDataSource<HdRprimCollection>;
/// Handle to rprim collection data source.
pub type HdRprimCollectionDataSourceHandle = Arc<HdRprimCollectionDataSource>;

/// Data source for Vec<Token> (TfTokenVector).
pub type HdTokenVectorDataSource = dyn HdTypedSampledDataSource<Vec<Token>>;
/// Handle to token vector data source.
pub type HdTokenVectorDataSourceHandle = Arc<HdTokenVectorDataSource>;

/// Data source for HdLegacyTaskFactory.
pub type HdLegacyTaskFactoryDataSource = dyn HdTypedSampledDataSource<HdLegacyTaskFactoryHandle>;
/// Handle to legacy task factory data source.
pub type HdLegacyTaskFactoryDataSourceHandle = Arc<HdLegacyTaskFactoryDataSource>;

/// Schema for legacy task - provides task factory and parameters.
///
/// Corresponds to C++ HdLegacyTaskSchema.
#[derive(Debug, Clone)]
pub struct HdLegacyTaskSchema {
    schema: HdSchema,
}

impl HdLegacyTaskSchema {
    /// Create from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves schema from parent container under "task" token.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&TASK) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema has a valid container.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get task factory data source.
    pub fn get_factory(&self) -> Option<HdLegacyTaskFactoryDataSourceHandle> {
        self.schema.get_typed(&FACTORY)
    }

    /// Parameters for task. Type depends on task type.
    /// Returns base handle - caller may use as_sampled() for get_value.
    pub fn get_parameters(&self) -> Option<HdDataSourceBaseHandle> {
        self.schema.get_container()?.get(&PARAMETERS)
    }

    /// Get rprim collection data source.
    pub fn get_collection(&self) -> Option<HdRprimCollectionDataSourceHandle> {
        self.schema.get_typed(&COLLECTION)
    }

    /// Get render tags (token vector).
    pub fn get_render_tags(&self) -> Option<HdTokenVectorDataSourceHandle> {
        self.schema.get_typed(&RENDER_TAGS)
    }

    /// Get schema identifying token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &TASK
    }

    /// Default locator for task schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TASK.clone()])
    }

    /// Locator for factory field.
    pub fn get_factory_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TASK.clone(), FACTORY.clone()])
    }

    /// Locator for parameters field.
    pub fn get_parameters_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TASK.clone(), PARAMETERS.clone()])
    }

    /// Locator for collection field.
    pub fn get_collection_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TASK.clone(), COLLECTION.clone()])
    }

    /// Locator for render tags field.
    pub fn get_render_tags_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TASK.clone(), RENDER_TAGS.clone()])
    }

    /// Build retained container with all provided fields.
    pub fn build_retained(
        factory: Option<HdLegacyTaskFactoryDataSourceHandle>,
        parameters: Option<HdSampledDataSourceHandle>,
        collection: Option<HdRprimCollectionDataSourceHandle>,
        render_tags: Option<HdTokenVectorDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::HdRetainedContainerDataSource;

        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();

        if let Some(f) = factory {
            entries.push((FACTORY.clone(), f as HdDataSourceBaseHandle));
        }
        if let Some(p) = parameters {
            entries.push((PARAMETERS.clone(), p as HdDataSourceBaseHandle));
        }
        if let Some(c) = collection {
            entries.push((COLLECTION.clone(), c as HdDataSourceBaseHandle));
        }
        if let Some(r) = render_tags {
            entries.push((RENDER_TAGS.clone(), r as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdLegacyTaskSchema.
#[derive(Default)]
pub struct HdLegacyTaskSchemaBuilder {
    factory: Option<HdLegacyTaskFactoryDataSourceHandle>,
    parameters: Option<crate::data_source::HdSampledDataSourceHandle>,
    collection: Option<HdRprimCollectionDataSourceHandle>,
    render_tags: Option<HdTokenVectorDataSourceHandle>,
}

impl HdLegacyTaskSchemaBuilder {
    /// Create empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set task factory.
    pub fn set_factory(mut self, v: HdLegacyTaskFactoryDataSourceHandle) -> Self {
        self.factory = Some(v);
        self
    }

    /// Set task parameters.
    pub fn set_parameters(mut self, v: crate::data_source::HdSampledDataSourceHandle) -> Self {
        self.parameters = Some(v);
        self
    }

    /// Set rprim collection.
    pub fn set_collection(mut self, v: HdRprimCollectionDataSourceHandle) -> Self {
        self.collection = Some(v);
        self
    }

    /// Set render tags.
    pub fn set_render_tags(mut self, v: HdTokenVectorDataSourceHandle) -> Self {
        self.render_tags = Some(v);
        self
    }

    /// Build container data source from accumulated fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdLegacyTaskSchema::build_retained(
            self.factory,
            self.parameters,
            self.collection,
            self.render_tags,
        )
    }
}
