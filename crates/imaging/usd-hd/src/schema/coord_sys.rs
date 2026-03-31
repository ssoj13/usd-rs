//! Coordinate system schema for Hydra.
//!
//! Defines a named coordinate system binding for shader coordinate
//! space transformations.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

// Schema tokens

/// Schema token for coordinate system
pub static COORD_SYS: Lazy<Token> = Lazy::new(|| Token::new("coordSys"));
/// Schema token for coordinate system name
pub static NAME: Lazy<Token> = Lazy::new(|| Token::new("name"));

// Typed data source

/// Data source for token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Shared handle to token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema representing a coordinate system.
///
/// Coordinate systems define named transformation spaces that can be
/// referenced by shaders. This schema provides the name binding.
///
/// Provides access to:
/// - `name` - Coordinate system name
///
/// # Location
///
/// Default locator: `coordSys`
#[derive(Debug, Clone)]
pub struct HdCoordSysSchema {
    schema: HdSchema,
}

impl HdCoordSysSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract coordinate system schema from parent container
    ///
    /// Returns empty schema if not found
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&COORD_SYS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Check if schema is defined (has valid container)
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get underlying container data source
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get coordinate system name
    pub fn get_name(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&NAME)
    }

    /// Get schema token for coordinate system
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &COORD_SYS
    }

    /// Get default data source locator for coordinate system
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[COORD_SYS.clone()])
    }

    /// Build retained container with coordinate system data
    ///
    /// # Arguments
    ///
    /// * `name` - Coordinate system name token
    pub fn build_retained(name: Option<HdTokenDataSourceHandle>) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = name {
            entries.push((NAME.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdCoordSysSchema
///
/// Provides fluent API for constructing coordinate system schemas.
#[derive(Default)]
pub struct HdCoordSysSchemaBuilder {
    /// Coordinate system name
    name: Option<HdTokenDataSourceHandle>,
}

impl HdCoordSysSchemaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set coordinate system name
    pub fn set_name(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.name = Some(v);
        self
    }

    /// Build container data source with configured values
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdCoordSysSchema::build_retained(self.name)
    }
}
