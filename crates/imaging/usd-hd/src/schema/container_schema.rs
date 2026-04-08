//! HdContainerSchema - Generic container with arbitrary child names.
//!
//! Corresponds to pxr/imaging/hd/containerSchema.h

use super::HdSchema;
use crate::data_source::HdContainerDataSourceHandle;
use std::sync::Arc;
use usd_tf::Token;

/// Schema for a container whose children have arbitrary names.
///
/// Unlike fixed schemas, this provides access to a container with
/// dynamic/arbitrary keys. Used as base for HdContainerOfSchemasSchema.
#[derive(Debug, Clone)]
pub struct HdContainerSchema {
    schema: HdSchema,
}

impl HdContainerSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get the names of all children in the container.
    pub fn get_names(&self) -> Vec<Token> {
        self.schema
            .get_container()
            .map(|c| c.get_names())
            .unwrap_or_default()
    }

    /// Get a child by name (returns base handle - caller downcasts as needed).
    pub fn get(&self, name: &Token) -> Option<crate::data_source::HdDataSourceBaseHandle> {
        self.schema.get_container()?.get(name)
    }

    /// Get typed child by name.
    pub fn get_typed<T>(&self, name: &Token) -> Option<Arc<T>>
    where
        T: ?Sized + 'static,
    {
        self.schema.get_typed(name)
    }

    /// Returns true if schema wraps a non-null container.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }
}

/// Schema for a container whose children are typed sampled data sources.
///
/// Corresponds to C++ HdContainerOfTypedSampledDataSourcesSchema<T>.
#[derive(Debug)]
pub struct HdContainerOfTypedSampledDataSourcesSchema<T: ?Sized> {
    container: HdContainerSchema,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ?Sized> Clone for HdContainerOfTypedSampledDataSourcesSchema<T> {
    fn clone(&self) -> Self {
        Self {
            container: self.container.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: ?Sized> HdContainerOfTypedSampledDataSourcesSchema<T> {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            container: HdContainerSchema::new(container),
            _marker: std::marker::PhantomData,
        }
    }

    /// Get typed child by name.
    pub fn get(&self, name: &Token) -> Option<Arc<T>>
    where
        T: 'static,
    {
        self.container.get_typed(name)
    }

    /// Get names of all children.
    pub fn get_names(&self) -> Vec<Token> {
        self.container.get_names()
    }

    /// Returns true if schema wraps a non-null container.
    pub fn is_defined(&self) -> bool {
        self.container.is_defined()
    }
}

/// Schema for a container whose children are schema-typed (container data sources).
///
/// Corresponds to C++ HdContainerOfSchemasSchema<Schema>.
#[derive(Debug, Clone)]
pub struct HdContainerOfSchemasSchema<S> {
    container: HdContainerSchema,
    _marker: std::marker::PhantomData<S>,
}

impl<S> HdContainerOfSchemasSchema<S>
where
    S: SchemaFromContainer,
{
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            container: HdContainerSchema::new(container),
            _marker: std::marker::PhantomData,
        }
    }

    /// Get schema-typed child by name.
    pub fn get(&self, name: &Token) -> S {
        let container_opt = self
            .container
            .get(name)
            .and_then(|base| crate::data_source::cast_to_container(&base));
        S::from_container(container_opt)
    }

    /// Get names of all children.
    pub fn get_names(&self) -> Vec<Token> {
        self.container.get_names()
    }

    /// Returns true if schema wraps a non-null container.
    pub fn is_defined(&self) -> bool {
        self.container.schema.is_defined()
    }
}

/// Trait for schemas that can be constructed from an optional container handle.
pub trait SchemaFromContainer {
    /// Construct schema from optional container (None = empty/undefined).
    fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self;
}
