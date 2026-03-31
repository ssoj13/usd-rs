//! Hydra schema for point cloud primitives.
//!
//! This module provides the [`HdPointsSchema`] which represents point cloud geometry
//! in the Hydra rendering system. Point clouds are collections of vertices rendered
//! as individual points, commonly used for particle systems, scanned data, and other
//! sparse geometric representations.
//!
//! The schema wraps an [`HdContainerDataSource`] and provides type-safe access to
//! point-related data following Hydra's data source conventions.
//!
//! # References
//!
//! - [OpenUSD HdPoints Class](https://openusd.org/docs/api/class_hd_points.html)
//! - [Hydra 2.0 Schema Architecture](https://openusd.org/dev/api/_page__hydra__getting__started__guide.html)

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

/// Schema token identifying the points primitive type.
///
/// This token is used as a key in data source containers to identify point cloud data.
pub static POINTS: Lazy<Token> = Lazy::new(|| Token::new("points"));

/// Schema wrapper for point cloud data sources.
///
/// Provides structured access to point cloud geometry within Hydra's data source
/// architecture. This schema represents the expected data format for point primitives,
/// enabling communication between scene indices and render delegates.
///
/// The schema is applied to an [`HdContainerDataSource`] and interprets its contents
/// according to Hydra's point cloud conventions.
#[derive(Debug, Clone)]
pub struct HdPointsSchema {
    schema: HdSchema,
}

impl HdPointsSchema {
    /// Creates a new points schema wrapping the given container data source.
    ///
    /// # Arguments
    ///
    /// * `container` - Container data source holding point cloud data
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves a points schema from a parent container using the standard locator.
    ///
    /// Looks up the child container at the [`POINTS`] token key and wraps it in a schema.
    /// Returns an empty schema if the points data is not found or cannot be downcast.
    ///
    /// # Arguments
    ///
    /// * `parent` - Parent container to search for points data
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&POINTS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns whether this schema wraps valid point cloud data.
    ///
    /// A schema is defined if it has a non-empty underlying container.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns a reference to the underlying container data source.
    ///
    /// Returns `None` if the schema is not defined (wraps an empty container).
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Returns the schema token identifying the points primitive type.
    ///
    /// This is the key used in data source containers to locate point cloud data.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &POINTS
    }

    /// Returns the default data source locator for points schemas.
    ///
    /// The locator consists of the [`POINTS`] token and is used to navigate
    /// to point data within container hierarchies.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[POINTS.clone()])
    }

    /// Builds an empty retained container for points data.
    ///
    /// Creates a new retained (owned) container data source with no initial contents.
    /// Useful for constructing point cloud data programmatically.
    pub fn build_retained() -> HdContainerDataSourceHandle {
        use crate::data_source::HdRetainedContainerDataSource;
        HdRetainedContainerDataSource::new_empty()
    }
}
