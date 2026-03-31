#![allow(dead_code)]
//! Render buffer schema for Hydra.
//!
//! Defines render buffer properties including dimensions, format,
//! and multi-sampling flag.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::Vec3i;
use usd_tf::Token;

/// Render buffer schema token
pub static RENDER_BUFFER: Lazy<Token> = Lazy::new(|| Token::new("renderBuffer"));
/// Buffer dimensions token
pub static DIMENSIONS: Lazy<Token> = Lazy::new(|| Token::new("dimensions"));
/// Buffer format token
pub static FORMAT: Lazy<Token> = Lazy::new(|| Token::new("format"));
/// Multi-sampled flag token
pub static MULTI_SAMPLED: Lazy<Token> = Lazy::new(|| Token::new("multiSampled"));

/// Data source for Vec3i values
pub type HdVec3iDataSource = dyn HdTypedSampledDataSource<Vec3i>;
/// Arc handle to Vec3i data source
pub type HdVec3iDataSourceHandle = Arc<HdVec3iDataSource>;

/// Data source for Token values (format)
pub type HdFormatDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to format data source
pub type HdFormatDataSourceHandle = Arc<HdFormatDataSource>;

/// Data source for bool values
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool>;
/// Arc handle to bool data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Schema representing render buffer configuration.
///
/// Provides access to:
/// - `dimensions` - Buffer dimensions (width, height, depth)
/// - `format` - Pixel format token
/// - `multiSampled` - Whether buffer uses multi-sampling
///
/// # Location
///
/// Default locator: `renderBuffer`
#[derive(Debug, Clone)]
pub struct HdRenderBufferSchema {
    schema: HdSchema,
}

impl HdRenderBufferSchema {
    /// Constructs a render buffer schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves render buffer schema from parent container at "renderBuffer" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&RENDER_BUFFER) {
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

    /// Gets buffer dimensions (width, height, depth).
    pub fn get_dimensions(&self) -> Option<HdVec3iDataSourceHandle> {
        self.schema.get_typed(&DIMENSIONS)
    }

    /// Gets pixel format token.
    pub fn get_format(&self) -> Option<HdFormatDataSourceHandle> {
        self.schema.get_typed(&FORMAT)
    }

    /// Gets multi-sampled flag.
    pub fn get_multi_sampled(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&MULTI_SAMPLED)
    }

    /// Returns the schema token for render buffer.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &RENDER_BUFFER
    }

    /// Returns the default locator for render buffer schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_BUFFER.clone()])
    }

    /// Returns the locator for dimensions.
    pub fn get_dimensions_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_BUFFER.clone(), DIMENSIONS.clone()])
    }

    /// Returns the locator for format.
    pub fn get_format_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_BUFFER.clone(), FORMAT.clone()])
    }

    /// Returns the locator for multi-sampled flag.
    pub fn get_multi_sampled_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_BUFFER.clone(), MULTI_SAMPLED.clone()])
    }

    /// Builds a retained container with render buffer parameters.
    ///
    /// # Parameters
    /// - `dimensions` - Buffer dimensions
    /// - `format` - Pixel format
    /// - `multi_sampled` - Multi-sampling flag
    pub fn build_retained(
        dimensions: Option<HdVec3iDataSourceHandle>,
        format: Option<HdFormatDataSourceHandle>,
        multi_sampled: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(d) = dimensions {
            entries.push((DIMENSIONS.clone(), d as HdDataSourceBaseHandle));
        }
        if let Some(f) = format {
            entries.push((FORMAT.clone(), f as HdDataSourceBaseHandle));
        }
        if let Some(m) = multi_sampled {
            entries.push((MULTI_SAMPLED.clone(), m as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdRenderBufferSchema using builder pattern.
#[derive(Default)]
pub struct HdRenderBufferSchemaBuilder {
    dimensions: Option<HdVec3iDataSourceHandle>,
    format: Option<HdFormatDataSourceHandle>,
    multi_sampled: Option<HdBoolDataSourceHandle>,
}

impl HdRenderBufferSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets buffer dimensions.
    pub fn set_dimensions(mut self, v: HdVec3iDataSourceHandle) -> Self {
        self.dimensions = Some(v);
        self
    }

    /// Sets pixel format.
    pub fn set_format(mut self, v: HdFormatDataSourceHandle) -> Self {
        self.format = Some(v);
        self
    }

    /// Sets multi-sampled flag.
    pub fn set_multi_sampled(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.multi_sampled = Some(v);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdRenderBufferSchema::build_retained(self.dimensions, self.format, self.multi_sampled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_buffer_schema_tokens() {
        assert_eq!(RENDER_BUFFER.as_str(), "renderBuffer");
        assert_eq!(DIMENSIONS.as_str(), "dimensions");
        assert_eq!(FORMAT.as_str(), "format");
        assert_eq!(MULTI_SAMPLED.as_str(), "multiSampled");
    }

    #[test]
    fn test_render_buffer_schema_locators() {
        let default_loc = HdRenderBufferSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let dims_loc = HdRenderBufferSchema::get_dimensions_locator();
        assert_eq!(dims_loc.len(), 2);

        let format_loc = HdRenderBufferSchema::get_format_locator();
        assert_eq!(format_loc.len(), 2);

        let ms_loc = HdRenderBufferSchema::get_multi_sampled_locator();
        assert_eq!(ms_loc.len(), 2);
    }
}
