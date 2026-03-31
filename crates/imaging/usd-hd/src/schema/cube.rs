//! Cube schema for Hydra.
//!
//! Defines implicit cube geometry with uniform or per-axis dimensions.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_tf::Token;

/// Cube schema token
pub static CUBE: LazyLock<Token> = LazyLock::new(|| Token::new("cube"));
/// Size token (uniform dimension)
pub static SIZE: LazyLock<Token> = LazyLock::new(|| Token::new("size"));

/// Data source for double values
pub type HdDoubleDataSource = dyn HdTypedSampledDataSource<f64>;
/// Arc handle to double data source
pub type HdDoubleDataSourceHandle = Arc<HdDoubleDataSource>;

/// Schema representing cube geometry.
///
/// Provides access to:
/// - `size` - Uniform cube size (default: 2.0)
///
/// # Location
///
/// Default locator: `cube`
#[derive(Debug, Clone)]
pub struct HdCubeSchema {
    schema: HdSchema,
}

impl HdCubeSchema {
    /// Constructs a cube schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves cube schema from parent container at "cube" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&CUBE) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema is non-empty.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Gets the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets uniform cube size.
    pub fn get_size(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&SIZE)
    }

    /// Returns the schema token for cube.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &CUBE
    }

    /// Returns the default locator for cube schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CUBE.clone()])
    }

    /// Returns the locator for size.
    pub fn get_size_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CUBE.clone(), SIZE.clone()])
    }

    /// Builds a retained container with cube parameters.
    ///
    /// # Parameters
    /// - `size` - Uniform cube size
    pub fn build_retained(size: Option<HdDoubleDataSourceHandle>) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(s) = size {
            entries.push((SIZE.clone(), s as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cube_tokens() {
        assert_eq!(CUBE.as_str(), "cube");
        assert_eq!(SIZE.as_str(), "size");
    }

    #[test]
    fn test_cube_locators() {
        let default_loc = HdCubeSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let size_loc = HdCubeSchema::get_size_locator();
        assert_eq!(size_loc.len(), 2);
    }
}
