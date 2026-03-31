//! HdExtComputationOutputSchema - Output of an ext computation.
//!
//! Port of pxr/imaging/hd/extComputationOutputSchema.h
//!
//! Describes an output's value type (HdTupleType).

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    HdTypedSampledDataSource,
};
use crate::types::HdTupleType;
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Data source for HdTupleType (output value type).
pub type HdTupleTypeDataSource = dyn HdTypedSampledDataSource<HdTupleType>;
/// Handle for HdTupleTypeDataSource.
pub type HdTupleTypeDataSourceHandle = Arc<HdTupleTypeDataSource>;

static VALUE_TYPE: Lazy<Token> = Lazy::new(|| Token::new("valueType"));

/// Schema for an ext computation output (value type descriptor).
///
/// Fields: valueType (HdTupleType)
#[derive(Debug, Clone)]
pub struct HdExtComputationOutputSchema {
    schema: HdSchema,
}

impl HdExtComputationOutputSchema {
    /// Creates schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Gets schema from parent container by child name.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle, name: &Token) -> Self {
        if let Some(child) = parent.get(name) {
            if let Some(container) = crate::data_source::cast_to_container(&child) {
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

    /// Gets the value type (tuple type descriptor).
    pub fn get_value_type(&self) -> Option<HdTupleTypeDataSourceHandle> {
        self.schema.get_typed(&VALUE_TYPE)
    }

    /// Builds a retained container with all fields.
    pub fn build_retained(
        value_type: Option<HdTupleTypeDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(v) = value_type {
            entries.push((VALUE_TYPE.clone(), v as HdDataSourceBaseHandle));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for ExtComputationOutput schema.
pub struct HdExtComputationOutputSchemaBuilder {
    value_type: Option<HdTupleTypeDataSourceHandle>,
}

impl HdExtComputationOutputSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self { value_type: None }
    }

    /// Sets the value type.
    pub fn set_value_type(mut self, v: HdTupleTypeDataSourceHandle) -> Self {
        self.value_type = Some(v);
        self
    }

    /// Builds the container.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdExtComputationOutputSchema::build_retained(self.value_type)
    }
}
