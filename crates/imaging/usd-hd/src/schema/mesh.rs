//! Mesh schema for Hydra.
//!
//! Defines mesh geometry including topology, subdivision scheme,
//! subdivision tags, and double-sided rendering flag.

use super::{HdMeshTopologySchema, HdSchema};
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

/// Mesh schema token
pub static MESH: Lazy<Token> = Lazy::new(|| Token::new("mesh"));
/// Mesh topology token
pub static TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("topology"));
/// Subdivision scheme token
pub static SUBDIVISION_SCHEME: Lazy<Token> = Lazy::new(|| Token::new("subdivisionScheme"));
/// Subdivision tags token
pub static SUBDIVISION_TAGS: Lazy<Token> = Lazy::new(|| Token::new("subdivisionTags"));
/// Double-sided rendering flag token
pub static DOUBLE_SIDED: Lazy<Token> = Lazy::new(|| Token::new("doubleSided"));

// Re-export typed data source handles (originally defined in type_defs,
// re-exported here for backward compatibility with downstream crates)
pub use crate::data_source::HdBoolDataSourceHandle;
pub use crate::data_source::HdTokenDataSourceHandle;

/// Schema representing mesh geometry.
///
/// Provides access to:
/// - `topology` - Mesh topology (face counts, indices, orientation)
/// - `subdivisionScheme` - Subdivision scheme (catmullClark, loop, bilinear, none)
/// - `subdivisionTags` - Subdivision tags (creases, corners, holes)
/// - `doubleSided` - Whether mesh is double-sided
///
/// # Location
///
/// Default locator: `mesh`
#[derive(Debug, Clone)]
pub struct HdMeshSchema {
    schema: HdSchema,
}

impl HdMeshSchema {
    /// Constructs a mesh schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves mesh schema from parent container at "mesh" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&MESH) {
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

    /// Gets mesh topology schema.
    pub fn get_topology(&self) -> Option<HdMeshTopologySchema> {
        if let Some(container) = self.schema.get_container() {
            let topo = HdMeshTopologySchema::get_from_parent(container);
            if topo.is_defined() {
                return Some(topo);
            }
        }
        None
    }

    /// Gets subdivision scheme (catmullClark, loop, bilinear, none).
    ///
    /// Uses `get_typed_retained` with adapter fallback so this works for
    /// both retained and attribute-backed data sources.
    pub fn get_subdivision_scheme(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained::<Token>(&SUBDIVISION_SCHEME)
    }

    /// Gets subdivision tags container (creases, corners, holes).
    pub fn get_subdivision_tags(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(&SUBDIVISION_TAGS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Gets double-sided flag.
    ///
    /// Uses `get_typed_retained` with adapter fallback so this works for
    /// both retained and attribute-backed data sources.
    pub fn get_double_sided(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed_retained::<bool>(&DOUBLE_SIDED)
    }

    /// Returns the schema token for mesh.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &MESH
    }

    /// Returns the default locator for mesh schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MESH.clone()])
    }

    /// Returns the locator for mesh topology.
    pub fn get_topology_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MESH.clone(), TOPOLOGY.clone()])
    }

    /// Returns the locator for subdivision scheme.
    pub fn get_subdivision_scheme_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MESH.clone(), SUBDIVISION_SCHEME.clone()])
    }

    /// Returns the locator for subdivision tags.
    pub fn get_subdivision_tags_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MESH.clone(), SUBDIVISION_TAGS.clone()])
    }

    /// Returns the locator for double-sided flag.
    pub fn get_double_sided_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MESH.clone(), DOUBLE_SIDED.clone()])
    }

    /// Builds a retained container with mesh parameters.
    ///
    /// # Parameters
    /// All mesh settings as optional data source handles.
    pub fn build_retained(
        topology: Option<HdContainerDataSourceHandle>,
        subdivision_scheme: Option<HdTokenDataSourceHandle>,
        subdivision_tags: Option<HdContainerDataSourceHandle>,
        double_sided: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(t) = topology {
            entries.push((TOPOLOGY.clone(), t as HdDataSourceBaseHandle));
        }
        if let Some(s) = subdivision_scheme {
            entries.push((SUBDIVISION_SCHEME.clone(), s as HdDataSourceBaseHandle));
        }
        if let Some(t) = subdivision_tags {
            entries.push((SUBDIVISION_TAGS.clone(), t as HdDataSourceBaseHandle));
        }
        if let Some(d) = double_sided {
            entries.push((DOUBLE_SIDED.clone(), d as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
