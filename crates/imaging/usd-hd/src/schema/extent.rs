//! Extent (bounding box) schema for Hydra primitives.
//!
//! Provides min/max coordinates defining axis-aligned bounding box.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::Vec3d;
use usd_tf::Token;

/// Schema name token: "extent"
pub static EXTENT: Lazy<Token> = Lazy::new(|| Token::new("extent"));

/// Field name token: "min" (minimum corner of bounding box)
pub static MIN: Lazy<Token> = Lazy::new(|| Token::new("min"));

/// Field name token: "max" (maximum corner of bounding box)
pub static MAX: Lazy<Token> = Lazy::new(|| Token::new("max"));

/// Handle to Vec3d data source for extent coordinates
pub type HdVec3dDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Vec3d> + Send + Sync>;

/// Schema representing axis-aligned bounding box extent.
///
/// Provides access to min and max corner coordinates.
///
/// # Location
///
/// Default locator: `extent`
#[derive(Debug, Clone)]
pub struct HdExtentSchema {
    /// Underlying schema container
    schema: HdSchema,
}

impl HdExtentSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract extent schema from parent container
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&EXTENT) {
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

    /// Get minimum corner of bounding box
    pub fn get_min(&self) -> Option<HdVec3dDataSourceHandle> {
        self.schema.get_typed(&MIN)
    }

    /// Get maximum corner of bounding box
    pub fn get_max(&self) -> Option<HdVec3dDataSourceHandle> {
        self.schema.get_typed(&MAX)
    }

    /// Get schema name token
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &EXTENT
    }

    /// Get default locator for extent data
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[EXTENT.clone()])
    }

    /// Build retained container with extent data
    pub fn build_retained(
        min: Option<HdVec3dDataSourceHandle>,
        max: Option<HdVec3dDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();
        if let Some(m) = min {
            entries.push((MIN.clone(), m as HdDataSourceBaseHandle));
        }
        if let Some(m) = max {
            entries.push((MAX.clone(), m as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
