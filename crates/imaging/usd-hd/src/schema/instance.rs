//! Instance schema for Hydra.
//!
//! Represents instance data pointing back to an instancer that manages
//! the prototype. Used for instance aggregation and data processing.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

// Schema tokens

/// Schema token for instance data
pub static INSTANCE: Lazy<Token> = Lazy::new(|| Token::new("instance"));
/// Schema token for instancer path
pub static INSTANCER: Lazy<Token> = Lazy::new(|| Token::new("instancer"));
/// Schema token for prototype index
pub static PROTOTYPE_INDEX: Lazy<Token> = Lazy::new(|| Token::new("prototypeIndex"));
/// Schema token for instance index
pub static INSTANCE_INDEX: Lazy<Token> = Lazy::new(|| Token::new("instanceIndex"));

// Typed data sources

/// Data source for path values
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path>;
/// Shared handle to path data source
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;

/// Data source for integer values
pub type HdIntDataSource = dyn HdTypedSampledDataSource<i32>;
/// Shared handle to integer data source
pub type HdIntDataSourceHandle = Arc<HdIntDataSource>;

/// Schema representing instance data.
///
/// This schema is the opposite of instancerTopology's "instanceLocations".
/// When the scene coalesces prims into multiple instances of a single prototype,
/// it inserts "instance" prims at the site of de-duplication. The instancer prim
/// uses "instanceLocations" to point back to all these instance prims.
///
/// Provides access to:
/// - `instancer` - Path to the instancer managing this instance
/// - `prototypeIndex` - Index into instancer's instanceIndices vector
/// - `instanceIndex` - Index into the int array within instanceIndices
///
/// Instance prims aren't directly useful for rendering but can be useful
/// for scene processing and data aggregation.
///
/// # Location
///
/// Default locator: `instance`
#[derive(Debug, Clone)]
pub struct HdInstanceSchema {
    schema: HdSchema,
}

impl HdInstanceSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract instance schema from parent container
    ///
    /// Returns empty schema if not found
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&INSTANCE) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
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

    /// Path to instancer managing this instance
    pub fn get_instancer(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&INSTANCER)
    }

    /// Index into instancer's instanceIndices vector (outer index)
    pub fn get_prototype_index(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&PROTOTYPE_INDEX)
    }

    /// Index into int array within instanceIndices (inner index)
    pub fn get_instance_index(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&INSTANCE_INDEX)
    }

    /// Get schema token for instance
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &INSTANCE
    }

    /// Get default data source locator for instance
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[INSTANCE.clone()])
    }

    /// Build retained container with instance data
    ///
    /// # Arguments
    ///
    /// * `instancer` - Path to instancer managing this instance
    /// * `prototype_index` - Index into instancer's prototypes array
    /// * `instance_index` - Index into instance indices array for this prototype
    pub fn build_retained(
        instancer: Option<HdPathDataSourceHandle>,
        prototype_index: Option<HdIntDataSourceHandle>,
        instance_index: Option<HdIntDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = instancer {
            entries.push((INSTANCER.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = prototype_index {
            entries.push((PROTOTYPE_INDEX.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = instance_index {
            entries.push((INSTANCE_INDEX.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdInstanceSchema
///
/// Provides fluent API for constructing instance schemas.
#[allow(dead_code)] // Ready for use when schema population is needed
#[derive(Default)]
pub struct HdInstanceSchemaBuilder {
    /// Instancer path
    instancer: Option<HdPathDataSourceHandle>,
    /// Prototype index
    prototype_index: Option<HdIntDataSourceHandle>,
    /// Instance index
    instance_index: Option<HdIntDataSourceHandle>,
}

#[allow(dead_code)]
impl HdInstanceSchemaBuilder {
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

    /// Set instance index
    pub fn set_instance_index(mut self, v: HdIntDataSourceHandle) -> Self {
        self.instance_index = Some(v);
        self
    }

    /// Build container data source with configured values
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdInstanceSchema::build_retained(self.instancer, self.prototype_index, self.instance_index)
    }
}
