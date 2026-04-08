//! Dependency schema for Hydra.
//!
//! Port of pxr/imaging/hd/dependencySchema.
//!
//! Describes a dependency: when locator A on prim P changes, locator B is affected.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
};
use once_cell::sync::Lazy;
use std::sync::Arc;

use crate::schema::ext_computation_input_computation::HdPathDataSourceHandle;

// Schema tokens
static DEPENDED_ON_PRIM_PATH: Lazy<usd_tf::Token> =
    Lazy::new(|| usd_tf::Token::new("dependedOnPrimPath"));
static DEPENDED_ON_DATASOURCE_LOCATOR: Lazy<usd_tf::Token> =
    Lazy::new(|| usd_tf::Token::new("dependedOnDataSourceLocator"));
static AFFECTED_DATASOURCE_LOCATOR: Lazy<usd_tf::Token> =
    Lazy::new(|| usd_tf::Token::new("affectedDataSourceLocator"));

/// Data source for HdDataSourceLocator values.
pub type HdLocatorDataSource =
    dyn crate::data_source::HdTypedSampledDataSource<crate::data_source::HdDataSourceLocator>;
/// Handle to locator data source.
pub type HdLocatorDataSourceHandle = Arc<HdLocatorDataSource>;

/// Schema representing a single dependency entry.
///
/// Fields:
/// - dependedOnPrimPath: Path of prim we depend on
/// - dependedOnDataSourceLocator: Locator on that prim we depend on
/// - affectedDataSourceLocator: Locator on our prim that gets invalidated
#[derive(Debug, Clone)]
pub struct HdDependencySchema {
    schema: HdSchema,
}

impl HdDependencySchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Returns true if this schema is applied on top of a non-null container.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get the prim path we depend on.
    pub fn get_depended_on_prim_path(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&DEPENDED_ON_PRIM_PATH)
    }

    /// Get the data source locator we depend on.
    pub fn get_depended_on_data_source_locator(&self) -> Option<HdLocatorDataSourceHandle> {
        self.schema.get_typed(&DEPENDED_ON_DATASOURCE_LOCATOR)
    }

    /// Get the data source locator that is affected (invalidated).
    pub fn get_affected_data_source_locator(&self) -> Option<HdLocatorDataSourceHandle> {
        self.schema.get_typed(&AFFECTED_DATASOURCE_LOCATOR)
    }

    /// Build retained container for a dependency.
    pub fn build_retained(
        depended_on_prim_path: Option<HdPathDataSourceHandle>,
        depended_on_data_source_locator: Option<HdLocatorDataSourceHandle>,
        affected_data_source_locator: Option<HdLocatorDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries = Vec::new();
        if let Some(v) = depended_on_prim_path {
            entries.push((DEPENDED_ON_PRIM_PATH.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = depended_on_data_source_locator {
            entries.push((
                DEPENDED_ON_DATASOURCE_LOCATOR.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = affected_data_source_locator {
            entries.push((
                AFFECTED_DATASOURCE_LOCATOR.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdDependencySchema.
#[derive(Default)]
pub struct HdDependencySchemaBuilder {
    depended_on_prim_path: Option<HdPathDataSourceHandle>,
    depended_on_data_source_locator: Option<HdLocatorDataSourceHandle>,
    affected_data_source_locator: Option<HdLocatorDataSourceHandle>,
}

impl HdDependencySchemaBuilder {
    /// Set the prim path we depend on.
    pub fn set_depended_on_prim_path(mut self, v: HdPathDataSourceHandle) -> Self {
        self.depended_on_prim_path = Some(v);
        self
    }

    /// Set the data source locator we depend on.
    pub fn set_depended_on_data_source_locator(mut self, v: HdLocatorDataSourceHandle) -> Self {
        self.depended_on_data_source_locator = Some(v);
        self
    }

    /// Set the data source locator that gets invalidated.
    pub fn set_affected_data_source_locator(mut self, v: HdLocatorDataSourceHandle) -> Self {
        self.affected_data_source_locator = Some(v);
        self
    }

    /// Build container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdDependencySchema::build_retained(
            self.depended_on_prim_path,
            self.depended_on_data_source_locator,
            self.affected_data_source_locator,
        )
    }
}
