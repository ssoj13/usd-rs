//! Visibility schema for Hydra primitives.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Schema token for visibility property
pub static VISIBILITY: Lazy<Token> = Lazy::new(|| Token::new("visibility"));

/// Data source for boolean values
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool>;
/// Shared handle to boolean data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Schema representing prim visibility state.
///
/// Controls whether a prim is visible in the scene. This is a simple
/// boolean flag that can be inherited by child prims.
///
/// # Location
///
/// Default locator: `visibility`
#[derive(Debug, Clone)]
pub struct HdVisibilitySchema {
    /// Underlying schema container
    schema: HdSchema,
}

impl HdVisibilitySchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract visibility schema from parent container
    ///
    /// Returns empty schema if not found
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&VISIBILITY) {
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

    /// Get visibility boolean data source
    ///
    /// Returns None if visibility is not set
    pub fn get_visibility(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&VISIBILITY)
    }

    /// Get schema token for visibility
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &VISIBILITY
    }

    /// Get default data source locator for visibility
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VISIBILITY.clone()])
    }

    /// Build retained container with visibility data
    ///
    /// # Arguments
    ///
    /// * `visibility` - Optional boolean data source for visibility state
    pub fn build_retained(
        visibility: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();
        if let Some(v) = visibility {
            entries.push((VISIBILITY.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
