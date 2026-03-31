#![allow(dead_code)]
//! Render pass schema for Hydra.
//!
//! Defines render pass properties including pass type and render source.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

/// Render pass schema token
pub static RENDER_PASS: Lazy<Token> = Lazy::new(|| Token::new("renderPass"));
/// Pass type token
pub static PASS_TYPE: Lazy<Token> = Lazy::new(|| Token::new("passType"));
/// Render source path token
pub static RENDER_SOURCE: Lazy<Token> = Lazy::new(|| Token::new("renderSource"));

/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Data source for Path values
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path>;
/// Arc handle to Path data source
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;

/// Schema representing render pass configuration.
///
/// Provides access to:
/// - `passType` - Type of render pass (e.g., "geometry", "shadow")
/// - `renderSource` - Path to source prim for rendering
///
/// # Location
///
/// Default locator: `renderPass`
#[derive(Debug, Clone)]
pub struct HdRenderPassSchema {
    schema: HdSchema,
}

impl HdRenderPassSchema {
    /// Constructs a render pass schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves render pass schema from parent container at "renderPass" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&RENDER_PASS) {
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

    /// Gets pass type token.
    pub fn get_pass_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&PASS_TYPE)
    }

    /// Gets render source path.
    pub fn get_render_source(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&RENDER_SOURCE)
    }

    /// Returns the schema token for render pass.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &RENDER_PASS
    }

    /// Returns the default locator for render pass schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_PASS.clone()])
    }

    /// Returns the locator for pass type.
    pub fn get_pass_type_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_PASS.clone(), PASS_TYPE.clone()])
    }

    /// Returns the locator for render source.
    pub fn get_render_source_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_PASS.clone(), RENDER_SOURCE.clone()])
    }

    /// Builds a retained container with render pass parameters.
    ///
    /// # Parameters
    /// - `pass_type` - Type of render pass
    /// - `render_source` - Source path for rendering
    pub fn build_retained(
        pass_type: Option<HdTokenDataSourceHandle>,
        render_source: Option<HdPathDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(pt) = pass_type {
            entries.push((PASS_TYPE.clone(), pt as HdDataSourceBaseHandle));
        }
        if let Some(rs) = render_source {
            entries.push((RENDER_SOURCE.clone(), rs as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdRenderPassSchema using builder pattern.
#[derive(Default)]
pub struct HdRenderPassSchemaBuilder {
    pass_type: Option<HdTokenDataSourceHandle>,
    render_source: Option<HdPathDataSourceHandle>,
}

impl HdRenderPassSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets pass type.
    pub fn set_pass_type(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.pass_type = Some(v);
        self
    }

    /// Sets render source path.
    pub fn set_render_source(mut self, v: HdPathDataSourceHandle) -> Self {
        self.render_source = Some(v);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdRenderPassSchema::build_retained(self.pass_type, self.render_source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_pass_schema_tokens() {
        assert_eq!(RENDER_PASS.as_str(), "renderPass");
        assert_eq!(PASS_TYPE.as_str(), "passType");
        assert_eq!(RENDER_SOURCE.as_str(), "renderSource");
    }

    #[test]
    fn test_render_pass_schema_locators() {
        let default_loc = HdRenderPassSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let pt_loc = HdRenderPassSchema::get_pass_type_locator();
        assert_eq!(pt_loc.len(), 2);

        let rs_loc = HdRenderPassSchema::get_render_source_locator();
        assert_eq!(rs_loc.len(), 2);
    }
}
