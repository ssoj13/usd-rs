
//! Dependencies schema for Hydra.
//!
//! Port of pxr/imaging/hd/dependenciesSchema.
//!
//! Container of dependency entries at locator __dependencies.

use super::{HdSchema, dependency::HdDependencySchema};
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

static DEPENDENCIES: Lazy<Token> = Lazy::new(|| Token::new("__dependencies"));

/// Entry in the dependencies container: (name, dependency schema).
pub type DependenciesEntry = (Token, HdDependencySchema);

/// Schema for the __dependencies container on a prim.
///
/// Each child is a named dependency (HdDependencySchema).
#[derive(Debug, Clone)]
pub struct HdDependenciesSchema {
    schema: HdSchema,
}

impl HdDependenciesSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get schema from parent container (prim data source).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&DEPENDENCIES) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &DEPENDENCIES
    }

    /// Get default locator for __dependencies.
    pub fn get_default_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::from_token(DEPENDENCIES.clone())
    }

    /// Returns true if this schema is applied on top of a non-null container.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get all dependency entries.
    pub fn get_entries(&self) -> Vec<DependenciesEntry> {
        let mut result = Vec::new();
        if let Some(container) = self.schema.get_container() {
            for name in container.get_names() {
                if let Some(child) = container.get(&name) {
                    if let Some(child_container) = cast_to_container(&child) {
                        result.push((name, HdDependencySchema::new(child_container)));
                    }
                }
            }
        }
        result
    }

    /// Build retained container from name-value pairs.
    pub fn build_retained(
        names: &[Token],
        values: &[Arc<dyn crate::data_source::HdDataSourceBase>],
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};
        assert_eq!(names.len(), values.len());
        let entries: Vec<_> = names
            .iter()
            .zip(values.iter())
            .map(|(n, v)| (n.clone(), v.clone() as HdDataSourceBaseHandle))
            .collect();
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
