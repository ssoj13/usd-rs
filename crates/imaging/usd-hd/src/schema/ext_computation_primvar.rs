
//! HdExtComputationPrimvarSchema - Primvar driven by ext computation.
//!
//! Port of pxr/imaging/hd/extComputationPrimvarSchema.h
//!
//! Describes a primvar whose value comes from an ext computation output.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

static SOURCE_COMPUTATION: Lazy<Token> = Lazy::new(|| Token::new("sourceComputation"));
static SOURCE_COMPUTATION_OUTPUT_NAME: Lazy<Token> =
    Lazy::new(|| Token::new("sourceComputationOutputName"));

/// Schema for a primvar that gets its value from an ext computation output.
///
/// Fields: sourceComputation (path), sourceComputationOutputName (token)
#[derive(Debug, Clone)]
pub struct HdExtComputationPrimvarSchema {
    schema: HdSchema,
}

impl HdExtComputationPrimvarSchema {
    /// Creates schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Creates an empty (undefined) schema.
    pub fn empty() -> Self {
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get schema from a parent container, extracting the named child.
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
    pub fn get_source_computation(&self) -> Option<Arc<dyn HdTypedSampledDataSource<Path>>> {
        self.schema.get_typed(&SOURCE_COMPUTATION)
    }

    /// Gets the source computation output name (token).
    pub fn get_source_computation_output_name(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Token>>> {
        self.schema.get_typed(&SOURCE_COMPUTATION_OUTPUT_NAME)
    }
}
