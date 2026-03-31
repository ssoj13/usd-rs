//! Plane schema for Hydra.
//!
//! Defines implicit plane geometry (rectangular surface).

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_tf::Token;

/// Plane schema token
pub static PLANE: LazyLock<Token> = LazyLock::new(|| Token::new("plane"));
/// Width token
pub static WIDTH: LazyLock<Token> = LazyLock::new(|| Token::new("width"));
/// Length token
pub static LENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("length"));
/// Axis token
pub static AXIS: LazyLock<Token> = LazyLock::new(|| Token::new("axis"));
/// Double-sided rendering flag token
pub static DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));

/// Data source for double values
pub type HdDoubleDataSource = dyn HdTypedSampledDataSource<f64>;
/// Arc handle to double data source
pub type HdDoubleDataSourceHandle = Arc<HdDoubleDataSource>;
/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;
/// Data source for bool values
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool>;
/// Arc handle to bool data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Schema representing plane geometry.
///
/// Provides access to:
/// - `width` - Plane width (default: 2.0)
/// - `length` - Plane length (default: 2.0)
/// - `axis` - Normal axis (X, Y, Z)
/// - `doubleSided` - Whether plane is double-sided
///
/// # Location
///
/// Default locator: `plane`
#[derive(Debug, Clone)]
pub struct HdPlaneSchema {
    schema: HdSchema,
}

impl HdPlaneSchema {
    /// Constructs a plane schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves plane schema from parent container at "plane" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&PLANE) {
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

    /// Gets plane width.
    pub fn get_width(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&WIDTH)
    }

    /// Gets plane length.
    pub fn get_length(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&LENGTH)
    }

    /// Gets normal axis (X, Y, Z).
    pub fn get_axis(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&AXIS)
    }

    /// Gets double-sided flag.
    pub fn get_double_sided(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&DOUBLE_SIDED)
    }

    /// Returns the schema token for plane.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &PLANE
    }

    /// Returns the default locator for plane schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PLANE.clone()])
    }

    /// Returns the locator for width.
    pub fn get_width_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PLANE.clone(), WIDTH.clone()])
    }

    /// Returns the locator for length.
    pub fn get_length_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PLANE.clone(), LENGTH.clone()])
    }

    /// Returns the locator for axis.
    pub fn get_axis_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PLANE.clone(), AXIS.clone()])
    }

    /// Returns the locator for double-sided flag.
    pub fn get_double_sided_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PLANE.clone(), DOUBLE_SIDED.clone()])
    }

    /// Builds a retained container with plane parameters.
    ///
    /// # Parameters
    /// - `width` - Plane width
    /// - `length` - Plane length
    /// - `axis` - Normal axis
    /// - `double_sided` - Whether plane is double-sided
    pub fn build_retained(
        width: Option<HdDoubleDataSourceHandle>,
        length: Option<HdDoubleDataSourceHandle>,
        axis: Option<HdTokenDataSourceHandle>,
        double_sided: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(w) = width {
            entries.push((WIDTH.clone(), w as HdDataSourceBaseHandle));
        }
        if let Some(l) = length {
            entries.push((LENGTH.clone(), l as HdDataSourceBaseHandle));
        }
        if let Some(a) = axis {
            entries.push((AXIS.clone(), a as HdDataSourceBaseHandle));
        }
        if let Some(d) = double_sided {
            entries.push((DOUBLE_SIDED.clone(), d as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_tokens() {
        assert_eq!(PLANE.as_str(), "plane");
        assert_eq!(WIDTH.as_str(), "width");
        assert_eq!(LENGTH.as_str(), "length");
        assert_eq!(AXIS.as_str(), "axis");
        assert_eq!(DOUBLE_SIDED.as_str(), "doubleSided");
    }

    #[test]
    fn test_plane_locators() {
        let default_loc = HdPlaneSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let width_loc = HdPlaneSchema::get_width_locator();
        assert_eq!(width_loc.len(), 2);

        let length_loc = HdPlaneSchema::get_length_locator();
        assert_eq!(length_loc.len(), 2);

        let axis_loc = HdPlaneSchema::get_axis_locator();
        assert_eq!(axis_loc.len(), 2);

        let double_sided_loc = HdPlaneSchema::get_double_sided_locator();
        assert_eq!(double_sided_loc.len(), 2);
    }
}
