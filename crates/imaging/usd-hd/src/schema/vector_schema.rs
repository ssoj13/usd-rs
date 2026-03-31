
//! HdVectorSchema - Wrapper for vector data source.
//!
//! Corresponds to pxr/imaging/hd/vectorSchema.h

use crate::data_source::{HdVectorDataSourceHandle, cast_to_container};

/// Schema wrapping a vector data source.
///
/// Provides access to indexed elements. Use HdVectorOfSchemasSchema for
/// vectors of schema-typed elements.
#[derive(Debug, Clone)]
pub struct HdVectorSchema {
    vector: Option<HdVectorDataSourceHandle>,
}

impl HdVectorSchema {
    /// Create from vector data source.
    pub fn new(vector: HdVectorDataSourceHandle) -> Self {
        Self {
            vector: Some(vector),
        }
    }

    /// Create empty schema.
    pub fn empty() -> Self {
        Self { vector: None }
    }

    /// Get the underlying vector data source.
    pub fn get_vector(&self) -> Option<HdVectorDataSourceHandle> {
        self.vector.clone()
    }

    /// Returns true if this schema wraps a non-null vector.
    pub fn is_defined(&self) -> bool {
        self.vector.is_some()
    }

    /// Number of elements in the vector.
    pub fn get_num_elements(&self) -> usize {
        self.vector
            .as_ref()
            .map(|v| v.get_num_elements())
            .unwrap_or(0)
    }

    /// Get element at index (returns base handle).
    pub fn get_element(&self, index: usize) -> Option<crate::data_source::HdDataSourceBaseHandle> {
        self.vector.as_ref()?.get_element(index)
    }
}

/// Schema for a vector of typed sampled data sources.
///
/// Corresponds to C++ HdVectorOfTypedSampledDataSourcesSchema<T>.
#[derive(Debug, Clone)]
pub struct HdVectorOfTypedSampledDataSourcesSchema<T: ?Sized> {
    vector: HdVectorSchema,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ?Sized> HdVectorOfTypedSampledDataSourcesSchema<T> {
    /// Create from vector data source.
    pub fn new(vector: HdVectorDataSourceHandle) -> Self {
        Self {
            vector: HdVectorSchema::new(vector),
            _marker: std::marker::PhantomData,
        }
    }

    /// Create empty schema.
    pub fn empty() -> Self {
        Self {
            vector: HdVectorSchema::empty(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Get element at index.
    ///
    /// Returns the base handle. For typed access to concrete types, use
    /// `get_vector()` and downcast elements as needed.
    pub fn get_element(&self, index: usize) -> Option<crate::data_source::HdDataSourceBaseHandle> {
        self.vector.get_element(index)
    }

    /// Get underlying vector.
    pub fn get_vector(&self) -> Option<HdVectorDataSourceHandle> {
        self.vector.get_vector()
    }

    /// Number of elements.
    pub fn get_num_elements(&self) -> usize {
        self.vector.get_num_elements()
    }

    /// Returns true if schema wraps a non-null vector.
    pub fn is_defined(&self) -> bool {
        self.vector.is_defined()
    }
}

/// Schema for a vector of schema-typed elements (containers).
///
/// Corresponds to C++ HdVectorOfSchemasSchema<Schema>.
#[derive(Debug, Clone)]
pub struct HdVectorOfSchemasSchema<S> {
    vector: HdVectorSchema,
    _marker: std::marker::PhantomData<S>,
}

impl<S> HdVectorOfSchemasSchema<S>
where
    S: super::container_schema::SchemaFromContainer,
{
    /// Create from vector data source.
    pub fn new(vector: HdVectorDataSourceHandle) -> Self {
        Self {
            vector: HdVectorSchema::new(vector),
            _marker: std::marker::PhantomData,
        }
    }

    /// Create empty schema.
    pub fn empty() -> Self {
        Self {
            vector: HdVectorSchema::empty(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Get schema-typed element at index.
    pub fn get_element(&self, index: usize) -> S {
        let base = self.vector.get_element(index);
        let container_opt = base.and_then(|b| cast_to_container(&b));
        S::from_container(container_opt)
    }

    /// Get underlying vector.
    pub fn get_vector(&self) -> Option<HdVectorDataSourceHandle> {
        self.vector.get_vector()
    }

    /// Number of elements.
    pub fn get_num_elements(&self) -> usize {
        self.vector.get_num_elements()
    }

    /// Returns true if schema wraps a non-null vector.
    pub fn is_defined(&self) -> bool {
        self.vector.is_defined()
    }
}
