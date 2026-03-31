//! Sphere schema for Hydra.
//!
//! Defines implicit sphere geometry with radius and orientation axis.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_tf::Token;

/// Sphere schema token
pub static SPHERE: LazyLock<Token> = LazyLock::new(|| Token::new("sphere"));
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

/// Schema representing sphere geometry.
///
/// Provides access to:
/// - `radius` - Sphere radius (default: 1.0)
/// - `axis` - Orientation axis (X, Y, Z)
///
/// # Location
///
/// Default locator: `sphere`
#[derive(Debug, Clone)]
pub struct HdSphereSchema {
    schema: HdSchema,
}

impl HdSphereSchema {
    /// Constructs a sphere schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves sphere schema from parent container at "sphere" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&SPHERE) {
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

    /// Gets sphere radius.
    pub fn get_radius(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&RADIUS)
    }

    /// Gets orientation axis (X, Y, Z).
    pub fn get_axis(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&AXIS)
    }

    /// Returns the schema token for sphere.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &SPHERE
    }

    /// Returns the default locator for sphere schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SPHERE.clone()])
    }

    /// Returns the locator for radius.
    pub fn get_radius_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SPHERE.clone(), RADIUS.clone()])
    }

    /// Returns the locator for axis.
    pub fn get_axis_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SPHERE.clone(), AXIS.clone()])
    }

    /// Builds a retained container with sphere parameters.
    ///
    /// # Parameters
    /// - `radius` - Sphere radius
    /// - `axis` - Orientation axis
    pub fn build_retained(
        radius: Option<HdDoubleDataSourceHandle>,
        axis: Option<HdTokenDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

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
    fn test_sphere_tokens() {
        assert_eq!(SPHERE.as_str(), "sphere");
        assert_eq!(RADIUS.as_str(), "radius");
        assert_eq!(AXIS.as_str(), "axis");
    }

    #[test]
    fn test_sphere_locators() {
        let default_loc = HdSphereSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let radius_loc = HdSphereSchema::get_radius_locator();
        assert_eq!(radius_loc.len(), 2);

        let axis_loc = HdSphereSchema::get_axis_locator();
        assert_eq!(axis_loc.len(), 2);
    }
}
