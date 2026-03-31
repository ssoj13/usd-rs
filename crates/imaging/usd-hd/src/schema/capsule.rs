//! Capsule schema for Hydra.
//!
//! Defines implicit capsule geometry (cylinder with hemispherical caps).

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_tf::Token;

/// Capsule schema token
pub static CAPSULE: LazyLock<Token> = LazyLock::new(|| Token::new("capsule"));
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

/// Schema representing capsule geometry.
///
/// Provides access to:
/// - `height` - Cylinder height excluding caps (default: 2.0)
/// - `radius` - Capsule radius (default: 0.5)
/// - `axis` - Orientation axis (X, Y, Z)
///
/// # Location
///
/// Default locator: `capsule`
#[derive(Debug, Clone)]
pub struct HdCapsuleSchema {
    schema: HdSchema,
}

impl HdCapsuleSchema {
    /// Constructs a capsule schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves capsule schema from parent container at "capsule" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&CAPSULE) {
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

    /// Gets capsule height (cylinder portion, excluding caps).
    pub fn get_height(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&HEIGHT)
    }

    /// Gets capsule radius.
    pub fn get_radius(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&RADIUS)
    }

    /// Gets orientation axis (X, Y, Z).
    pub fn get_axis(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&AXIS)
    }

    /// Returns the schema token for capsule.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &CAPSULE
    }

    /// Returns the default locator for capsule schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAPSULE.clone()])
    }

    /// Returns the locator for height.
    pub fn get_height_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAPSULE.clone(), HEIGHT.clone()])
    }

    /// Returns the locator for radius.
    pub fn get_radius_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAPSULE.clone(), RADIUS.clone()])
    }

    /// Returns the locator for axis.
    pub fn get_axis_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAPSULE.clone(), AXIS.clone()])
    }

    /// Builds a retained container with capsule parameters.
    ///
    /// # Parameters
    /// - `height` - Cylinder height excluding caps
    /// - `radius` - Capsule radius
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
    fn test_capsule_tokens() {
        assert_eq!(CAPSULE.as_str(), "capsule");
        assert_eq!(HEIGHT.as_str(), "height");
        assert_eq!(RADIUS.as_str(), "radius");
        assert_eq!(AXIS.as_str(), "axis");
    }

    #[test]
    fn test_capsule_locators() {
        let default_loc = HdCapsuleSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let height_loc = HdCapsuleSchema::get_height_locator();
        assert_eq!(height_loc.len(), 2);

        let radius_loc = HdCapsuleSchema::get_radius_locator();
        assert_eq!(radius_loc.len(), 2);

        let axis_loc = HdCapsuleSchema::get_axis_locator();
        assert_eq!(axis_loc.len(), 2);
    }
}
