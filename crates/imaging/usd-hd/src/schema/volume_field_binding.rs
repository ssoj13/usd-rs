//! Volume field binding schema for Hydra.
//!
//! Defines bindings between volume fields and their targets.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_sdf::Path;
use usd_tf::Token;

/// Volume field binding schema token
pub static VOLUME_FIELD_BINDING: LazyLock<Token> =
    LazyLock::new(|| Token::new("volumeFieldBinding"));

/// Data source for Path values
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path>;
/// Arc handle to Path data source
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;

/// Schema representing volume field bindings.
///
/// This schema is a container where each child is a path data source
/// representing a binding from a field name to a field path.
///
/// # Location
///
/// Default locator: `volumeFieldBinding`
#[derive(Debug, Clone)]
pub struct HdVolumeFieldBindingSchema {
    schema: HdSchema,
}

impl HdVolumeFieldBindingSchema {
    /// Constructs a volume field binding schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves volume field binding schema from parent container at "volumeFieldBinding" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&VOLUME_FIELD_BINDING) {
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

    /// Gets all volume field binding names.
    pub fn get_volume_field_binding_names(&self) -> Vec<Token> {
        if let Some(container) = self.schema.get_container() {
            return container.get_names();
        }
        Vec::new()
    }

    /// Gets a specific volume field binding by name.
    pub fn get_volume_field_binding(&self, name: &Token) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(name)
    }

    /// Returns the schema token for volume field binding.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &VOLUME_FIELD_BINDING
    }

    /// Returns the default locator for volume field binding schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[VOLUME_FIELD_BINDING.clone()])
    }

    /// Builds a retained container with volume field binding parameters.
    ///
    /// # Parameters
    /// - `names` - Field names
    /// - `values` - Corresponding path data sources
    pub fn build_retained(
        names: &[Token],
        values: &[HdDataSourceBaseHandle],
    ) -> HdContainerDataSourceHandle {
        if names.len() != values.len() {
            // Return empty container if lengths don't match
            return HdRetainedContainerDataSource::from_entries(&[]);
        }

        let entries: Vec<(Token, HdDataSourceBaseHandle)> = names
            .iter()
            .zip(values.iter())
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_field_binding_schema_empty() {
        let empty_container: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[]);
        let schema = HdVolumeFieldBindingSchema::get_from_parent(&empty_container);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_volume_field_binding_schema_token() {
        assert_eq!(VOLUME_FIELD_BINDING.as_str(), "volumeFieldBinding");
    }

    #[test]
    fn test_volume_field_binding_schema_locator() {
        let default_loc = HdVolumeFieldBindingSchema::get_default_locator();
        assert_eq!(default_loc.elements().len(), 1);
    }

    #[test]
    fn test_volume_field_binding_names() {
        let empty_container: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[]);
        let schema = HdVolumeFieldBindingSchema::new(empty_container);
        let names = schema.get_volume_field_binding_names();
        assert_eq!(names.len(), 0);
    }
}
