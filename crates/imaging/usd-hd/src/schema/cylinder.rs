//! Cylinder schema for Hydra.
//!
//! Defines implicit cylinder geometry with circular cross-section.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_tf::Token;

/// Cylinder schema token
pub static CYLINDER: LazyLock<Token> = LazyLock::new(|| Token::new("cylinder"));
/// Height token
pub static HEIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("height"));
/// Radius token
pub static RADIUS: LazyLock<Token> = LazyLock::new(|| Token::new("radius"));
/// Axis token
pub static AXIS: LazyLock<Token> = LazyLock::new(|| Token::new("axis"));

/// Data source for double values
pub type HdDoubleDataSource = dyn HdTypedSampledDataSource<f64>;
/// Arc handle to double data source
pub type HdDoubleDataSourceHandle = Arc<HdDoubleDataSource>;
/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema representing cylinder geometry.
///
/// Provides access to:
/// - `height` - Cylinder height (default: 2.0)
/// - `radius` - Cylinder radius (default: 0.5)
/// - `axis` - Orientation axis (X, Y, Z)
///
/// # Location
///
/// Default locator: `cylinder`
#[derive(Debug, Clone)]
pub struct HdCylinderSchema {
    schema: HdSchema,
}

impl HdCylinderSchema {
    /// Constructs a cylinder schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves cylinder schema from parent container at "cylinder" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&CYLINDER) {
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

    /// Gets cylinder height.
    pub fn get_height(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&HEIGHT)
    }

    /// Gets cylinder radius.
    pub fn get_radius(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&RADIUS)
    }

    /// Gets orientation axis (X, Y, Z).
    pub fn get_axis(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&AXIS)
    }

    /// Returns the schema token for cylinder.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &CYLINDER
    }

    /// Returns the default locator for cylinder schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CYLINDER.clone()])
    }

    /// Returns the locator for height.
    pub fn get_height_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CYLINDER.clone(), HEIGHT.clone()])
    }

    /// Returns the locator for radius.
    pub fn get_radius_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CYLINDER.clone(), RADIUS.clone()])
    }

    /// Returns the locator for axis.
    pub fn get_axis_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CYLINDER.clone(), AXIS.clone()])
    }

    /// Builds a retained container with cylinder parameters.
    ///
    /// # Parameters
    /// - `height` - Cylinder height
    /// - `radius` - Cylinder radius
    /// - `axis` - Orientation axis
    pub fn build_retained(
        height: Option<HdDoubleDataSourceHandle>,
        radius: Option<HdDoubleDataSourceHandle>,
        axis: Option<HdTokenDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(h) = height {
            entries.push((HEIGHT.clone(), h as HdDataSourceBaseHandle));
        }
        if let Some(r) = radius {
            entries.push((RADIUS.clone(), r as HdDataSourceBaseHandle));
        }
        if let Some(a) = axis {
            entries.push((AXIS.clone(), a as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cylinder_tokens() {
        assert_eq!(CYLINDER.as_str(), "cylinder");
        assert_eq!(HEIGHT.as_str(), "height");
        assert_eq!(RADIUS.as_str(), "radius");
        assert_eq!(AXIS.as_str(), "axis");
    }

    #[test]
    fn test_cylinder_locators() {
        let default_loc = HdCylinderSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let height_loc = HdCylinderSchema::get_height_locator();
        assert_eq!(height_loc.len(), 2);

        let radius_loc = HdCylinderSchema::get_radius_locator();
        assert_eq!(radius_loc.len(), 2);

        let axis_loc = HdCylinderSchema::get_axis_locator();
        assert_eq!(axis_loc.len(), 2);
    }
}
