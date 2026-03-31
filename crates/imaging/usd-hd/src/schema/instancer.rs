//! Instancer topology schema for Hydra.
//!
//! Defines how prims are duplicated through instancing, including prototypes,
//! instance indices, mask, and instance locations.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

// Schema tokens

/// Schema token for instancer topology
pub static INSTANCER_TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("instancerTopology"));
/// Schema token for prototype paths array
pub static PROTOTYPES: Lazy<Token> = Lazy::new(|| Token::new("prototypes"));
/// Schema token for instance indices container
pub static INSTANCE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("instanceIndices"));
/// Schema token for instance mask array
pub static MASK: Lazy<Token> = Lazy::new(|| Token::new("mask"));
/// Schema token for instance locations array
pub static INSTANCE_LOCATIONS: Lazy<Token> = Lazy::new(|| Token::new("instanceLocations"));

// Typed data sources

/// Data source for path arrays
pub type HdPathArrayDataSource = dyn HdTypedSampledDataSource<Vec<Path>>;
/// Shared handle to path array data source
pub type HdPathArrayDataSourceHandle = Arc<HdPathArrayDataSource>;

/// Data source for boolean arrays
pub type HdBoolArrayDataSource = dyn HdTypedSampledDataSource<Vec<bool>>;
/// Shared handle to boolean array data source
pub type HdBoolArrayDataSourceHandle = Arc<HdBoolArrayDataSource>;

/// Schema representing instancer topology.
///
/// An instancer causes other prims to be duplicated. It holds:
/// - Instancer topology (how prims are duplicated)
/// - Instance-rate data (data varying per instance)
///
/// Provides access to:
/// - `prototypes` - Array of prototype paths to be instanced (e.g., ["/A", "/B"])
/// - `instanceIndices` - Nested array: per prototype, array of instance indices
///   Example: [[0,2], [1]] means draw /A twice (indices 0,2), /B once (index 1)
/// - `mask` - Boolean array to deactivate instances (empty = all true)
/// - `instanceLocations` - For implicit instancing: original paths of deduplicated prims
///
/// # Instancing Modes
///
/// 1. **Explicit instancing**: Prim wants to draw subtree at array of locations (data expansion)
/// 2. **Implicit instancing**: Identical prims are deduplicated and replaced with instancer (data coalescing)
///
/// For implicit instancing, instanceLocations stores original paths (e.g., /X, /Y)
/// while prototypes stores deduplicated paths (e.g., /_Prototype/Cube).
///
/// # Location
///
/// Default locator: `instancerTopology`
#[derive(Debug, Clone)]
pub struct HdInstancerTopologySchema {
    schema: HdSchema,
}

impl HdInstancerTopologySchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract instancer topology schema from parent container
    ///
    /// Returns empty schema if not found
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&INSTANCER_TOPOLOGY) {
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

    /// Get array of prototype paths
    pub fn get_prototypes(&self) -> Option<HdPathArrayDataSourceHandle> {
        self.schema.get_typed(&PROTOTYPES)
    }

    /// Get instance indices container (vector of int arrays, one per prototype)
    pub fn get_instance_indices(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&INSTANCE_INDICES) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Get mask array (boolean array to deactivate instances)
    pub fn get_mask(&self) -> Option<HdBoolArrayDataSourceHandle> {
        self.schema.get_typed(&MASK)
    }

    /// Get instance locations (for implicit instancing: original prim paths)
    pub fn get_instance_locations(&self) -> Option<HdPathArrayDataSourceHandle> {
        self.schema.get_typed(&INSTANCE_LOCATIONS)
    }

    /// Get schema token for instancer topology
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &INSTANCER_TOPOLOGY
    }

    /// Get default data source locator for instancer topology
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[INSTANCER_TOPOLOGY.clone()])
    }

    /// Build retained container with instancer topology data
    ///
    /// # Arguments
    ///
    /// * `prototypes` - Array of prototype paths to be instanced
    /// * `instance_indices` - Container with per-prototype instance index arrays
    /// * `mask` - Boolean array to deactivate specific instances
    /// * `instance_locations` - Original prim paths for implicit instancing
    pub fn build_retained(
        prototypes: Option<HdPathArrayDataSourceHandle>,
        instance_indices: Option<HdContainerDataSourceHandle>,
        mask: Option<HdBoolArrayDataSourceHandle>,
        instance_locations: Option<HdPathArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = prototypes {
            entries.push((PROTOTYPES.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = instance_indices {
            entries.push((INSTANCE_INDICES.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = mask {
            entries.push((MASK.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = instance_locations {
            entries.push((INSTANCE_LOCATIONS.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdInstancerTopologySchema
///
/// Provides fluent API for constructing instancer topology schemas.
#[allow(dead_code)] // Ready for use when schema population is needed
#[derive(Default)]
pub struct HdInstancerTopologySchemaBuilder {
    /// Prototype paths array
    prototypes: Option<HdPathArrayDataSourceHandle>,
    /// Instance indices container
    instance_indices: Option<HdContainerDataSourceHandle>,
    /// Instance mask array
    mask: Option<HdBoolArrayDataSourceHandle>,
    /// Instance locations array
    instance_locations: Option<HdPathArrayDataSourceHandle>,
}

#[allow(dead_code)]
impl HdInstancerTopologySchemaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set prototype paths array
    pub fn set_prototypes(mut self, v: HdPathArrayDataSourceHandle) -> Self {
        self.prototypes = Some(v);
        self
    }

    /// Set instance indices container
    pub fn set_instance_indices(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.instance_indices = Some(v);
        self
    }

    /// Set instance mask array
    pub fn set_mask(mut self, v: HdBoolArrayDataSourceHandle) -> Self {
        self.mask = Some(v);
        self
    }

    /// Set instance locations array
    pub fn set_instance_locations(mut self, v: HdPathArrayDataSourceHandle) -> Self {
        self.instance_locations = Some(v);
        self
    }

    /// Build container data source with configured values
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdInstancerTopologySchema::build_retained(
            self.prototypes,
            self.instance_indices,
            self.mask,
            self.instance_locations,
        )
    }
}
