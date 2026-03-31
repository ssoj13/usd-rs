//! Purpose schema for Hydra primitives.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

pub static PURPOSE: Lazy<Token> = Lazy::new(|| Token::new("purpose"));
/// Token for default purpose value.
#[allow(dead_code)]
pub static DEFAULT: Lazy<Token> = Lazy::new(|| Token::new("default"));
/// Token for render purpose (visible in final renders).
#[allow(dead_code)]
pub static RENDER: Lazy<Token> = Lazy::new(|| Token::new("render"));
/// Token for guide purpose (visible only as guides/helpers).
#[allow(dead_code)]
pub static GUIDE: Lazy<Token> = Lazy::new(|| Token::new("guide"));
/// Token for proxy purpose (low-resolution stand-in).
#[allow(dead_code)]
pub static PROXY: Lazy<Token> = Lazy::new(|| Token::new("proxy"));

pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema for primitive purpose classification.
///
/// Purpose defines how a primitive should be used:
/// - `default`: Regular geometry
/// - `render`: Visible in final renders
/// - `guide`: Visible only as guides/helpers
/// - `proxy`: Low-resolution stand-in geometry
#[derive(Debug, Clone)]
pub struct HdPurposeSchema {
    schema: HdSchema,
}

impl HdPurposeSchema {
    /// Creates a new purpose schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves the schema from a parent container data source.
    ///
    /// Looks for a child container under the `purpose` token.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&PURPOSE) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Checks if the schema is defined (has a valid container).
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Returns the purpose token (default, render, guide, or proxy).
    pub fn get_purpose(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&PURPOSE)
    }

    /// Returns the schema's identifying token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &PURPOSE
    }

    /// Returns the default data source locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PURPOSE.clone()])
    }

    /// Builds a retained container data source with the specified purpose.
    ///
    /// This is a factory method that constructs a container with purpose data.
    pub fn build_retained(purpose: Option<HdTokenDataSourceHandle>) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();
        if let Some(p) = purpose {
            entries.push((PURPOSE.clone(), p as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
