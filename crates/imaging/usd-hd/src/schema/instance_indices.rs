//! Instance indices schema for Hydra.
//!
//! Represents instance indices mapping for a specific prototype.

use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdTypedSampledDataSource};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

// Schema tokens

/// Schema token for instancer path
pub static INSTANCER: Lazy<Token> = Lazy::new(|| Token::new("instancer"));
/// Schema token for prototype index
pub static PROTOTYPE_INDEX: Lazy<Token> = Lazy::new(|| Token::new("prototypeIndex"));
/// Schema token for instance indices
pub static INSTANCE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("instanceIndices"));

// Typed data sources

/// Data source for path values
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path>;
/// Shared handle to path data source
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;

/// Data source for integer values
pub type HdIntDataSource = dyn HdTypedSampledDataSource<i32>;
/// Shared handle to integer data source
pub type HdIntDataSourceHandle = Arc<HdIntDataSource>;

/// Data source for integer arrays
pub type HdIntArrayDataSource = dyn HdTypedSampledDataSource<Vec<i32>>;
/// Shared handle to integer array data source
pub type HdIntArrayDataSourceHandle = Arc<HdIntArrayDataSource>;

/// Schema representing instance indices.
///
/// Provides access to:
/// - `instancer` - Path to the instancer prim
/// - `prototypeIndex` - Index into instancer's prototypes array
/// - `instanceIndices` - Array of instance indices for this prototype
///
/// # Location
///
/// Used within instancer topology's instanceIndices container
#[derive(Debug, Clone)]
pub struct HdInstanceIndicesSchema {
    schema: HdSchema,
}

impl HdInstanceIndicesSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Check if schema is defined (has valid container)
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get underlying container data source
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get instancer path
    pub fn get_instancer(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&INSTANCER)
    }

    /// Get prototype index
    pub fn get_prototype_index(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&PROTOTYPE_INDEX)
    }

    /// Get instance indices array
    pub fn get_instance_indices(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&INSTANCE_INDICES)
    }

    /// Build retained container with instance indices data
    ///
    /// # Arguments
    ///
    /// * `instancer` - Path to instancer prim
    /// * `prototype_index` - Index into instancer's prototypes array
    /// * `instance_indices` - Array of instance indices
    pub fn build_retained(
        instancer: Option<HdPathDataSourceHandle>,
        prototype_index: Option<HdIntDataSourceHandle>,
        instance_indices: Option<HdIntArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = instancer {
            entries.push((INSTANCER.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = prototype_index {
            entries.push((PROTOTYPE_INDEX.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = instance_indices {
            entries.push((INSTANCE_INDICES.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdInstanceIndicesSchema
///
/// Provides fluent API for constructing instance indices schemas.
#[allow(dead_code)]
#[derive(Default)]
pub struct HdInstanceIndicesSchemaBuilder {
    /// Instancer path
    instancer: Option<HdPathDataSourceHandle>,
    /// Prototype index
    prototype_index: Option<HdIntDataSourceHandle>,
    /// Instance indices array
    instance_indices: Option<HdIntArrayDataSourceHandle>,
}

#[allow(dead_code)]
impl HdInstanceIndicesSchemaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set instancer path
    pub fn set_instancer(mut self, v: HdPathDataSourceHandle) -> Self {
        self.instancer = Some(v);
        self
    }

    /// Set prototype index
    pub fn set_prototype_index(mut self, v: HdIntDataSourceHandle) -> Self {
        self.prototype_index = Some(v);
        self
    }

    /// Set instance indices array
    pub fn set_instance_indices(mut self, v: HdIntArrayDataSourceHandle) -> Self {
        self.instance_indices = Some(v);
        self
    }

    /// Build container data source with configured values
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdInstanceIndicesSchema::build_retained(
            self.instancer,
            self.prototype_index,
            self.instance_indices,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(INSTANCER.as_str(), "instancer");
        assert_eq!(PROTOTYPE_INDEX.as_str(), "prototypeIndex");
        assert_eq!(INSTANCE_INDICES.as_str(), "instanceIndices");
    }

    #[test]
    fn test_empty_schema() {
        let schema = HdInstanceIndicesSchema {
            schema: HdSchema::empty(),
        };
        assert!(!schema.is_defined());
        assert!(schema.get_container().is_none());
    }

    #[test]
    fn test_build_retained() {
        let container = HdInstanceIndicesSchema::build_retained(None, None, None);
        assert!(container.get_names().is_empty());
    }

    #[test]
    fn test_builder() {
        let builder = HdInstanceIndicesSchemaBuilder::new();
        let container = builder.build();
        assert!(container.get_names().is_empty());
    }
}
