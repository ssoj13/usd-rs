//! HdExtComputationInputComputationSchema - Input from another computation.
//!
//! Port of pxr/imaging/hd/extComputationInputComputationSchema.h
//!
//! Describes an input that comes from another ext computation's output.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

/// Data source for SdfPath.
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path>;
/// Handle for HdPathDataSource.
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;

static SOURCE_COMPUTATION: Lazy<Token> = Lazy::new(|| Token::new("sourceComputation"));
static SOURCE_COMPUTATION_OUTPUT_NAME: Lazy<Token> =
    Lazy::new(|| Token::new("sourceComputationOutputName"));

/// Schema for an input that references another computation's output.
///
/// Fields: sourceComputation (path), sourceComputationOutputName (token)
#[derive(Debug, Clone)]
pub struct HdExtComputationInputComputationSchema {
    schema: HdSchema,
}

impl HdExtComputationInputComputationSchema {
    /// Creates schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get schema from a parent container, extracting the named child.
    /// Used when iterating over inputComputations container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle, name: &Token) -> Self {
        if let Some(child) = parent.get(name) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema has valid data.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Gets the source computation path.
    pub fn get_source_computation(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&SOURCE_COMPUTATION)
    }

    /// Gets the source computation output name (token).
    pub fn get_source_computation_output_name(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Token>>> {
        self.schema.get_typed(&SOURCE_COMPUTATION_OUTPUT_NAME)
    }

    /// Builds a retained container with all fields.
    pub fn build_retained(
        source_computation: Option<HdPathDataSourceHandle>,
        source_computation_output_name: Option<Arc<dyn HdTypedSampledDataSource<Token>>>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(v) = source_computation {
            entries.push((SOURCE_COMPUTATION.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = source_computation_output_name {
            entries.push((
                SOURCE_COMPUTATION_OUTPUT_NAME.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for ExtComputationInputComputation schema.
pub struct HdExtComputationInputComputationSchemaBuilder {
    source_computation: Option<HdPathDataSourceHandle>,
    source_computation_output_name: Option<Arc<dyn HdTypedSampledDataSource<Token>>>,
}

impl HdExtComputationInputComputationSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            source_computation: None,
            source_computation_output_name: None,
        }
    }

    /// Sets the source computation path.
    pub fn set_source_computation(mut self, v: HdPathDataSourceHandle) -> Self {
        self.source_computation = Some(v);
        self
    }

    /// Sets the source computation output name.
    pub fn set_source_computation_output_name(
        mut self,
        v: Arc<dyn HdTypedSampledDataSource<Token>>,
    ) -> Self {
        self.source_computation_output_name = Some(v);
        self
    }

    /// Builds the container.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdExtComputationInputComputationSchema::build_retained(
            self.source_computation,
            self.source_computation_output_name,
        )
    }
}
