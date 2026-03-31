//! Material network schema for Hydra.
//!
//! This module provides the [`HdMaterialNetworkSchema`] which represents a complete
//! material network graph, including all shader nodes, terminal connections, and the
//! public material interface.
//!
//! # Overview
//!
//! A material network is a directed acyclic graph of shader nodes that defines
//! how a material is computed. The network includes:
//!
//! - **Nodes**: The shader nodes in the graph (each is an [`HdMaterialNodeSchema`](super::material_node::HdMaterialNodeSchema))
//! - **Terminals**: Output connections for surface, displacement, and volume shaders
//! - **Interface**: Public parameters exposed to users/artists
//! - **Config**: Renderer-specific configuration data
//!
//! The network topology is defined by connections between nodes, where each node's
//! inputs reference outputs from upstream nodes.
//!
//! # USD Reference
//!
//! This corresponds to OpenUSD's `HdMaterialNetworkSchema`:
//! - USD: `pxr/imaging/hd/materialNetworkSchema.h`
//! - Docs: <https://openusd.org/release/api/class_hd_material_network_schema.html>
//!
//! # Example
//!
//! ```ignore
//! use usd_hd::schema::material_network::*;
//!
//! // Build a material network
//! let network = HdMaterialNetworkSchemaBuilder::new()
//!     .set_nodes(nodes_container)
//!     .set_terminals(terminals_container)
//!     .set_interface(interface_container)
//!     .build();
//!
//! // Access the network
//! let schema = HdMaterialNetworkSchema::new(network);
//! if let Some(nodes) = schema.get_nodes() {
//!     // Process shader nodes
//! }
//! ```

use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

/// Schema locator token for the nodes container.
///
/// Maps node names to [`HdMaterialNodeSchema`](super::material_node::HdMaterialNodeSchema) containers.
pub static NODES: Lazy<Token> = Lazy::new(|| Token::new("nodes"));

/// Schema locator token for the terminals container.
///
/// Maps terminal names ("surface", "displacement", "volume") to connection descriptors.
pub static TERMINALS: Lazy<Token> = Lazy::new(|| Token::new("terminals"));

/// Schema locator token for the material interface.
///
/// Contains the public parameters exposed to users/artists.
pub static INTERFACE: Lazy<Token> = Lazy::new(|| Token::new("interface"));

/// Schema locator token for renderer configuration.
///
/// Holds renderer-specific configuration data.
pub static CONFIG: Lazy<Token> = Lazy::new(|| Token::new("config"));

/// Schema representing a complete material network in Hydra.
///
/// This schema wraps a container data source that defines a material as a
/// directed acyclic graph of shader nodes. The network describes how material
/// properties are computed through node connections.
///
/// # Schema Fields
///
/// - **nodes**: [`HdContainerDataSourceHandle`] - Maps node names to MaterialNode schemas
/// - **terminals**: [`HdContainerDataSourceHandle`] - Maps terminal types to output connections
///   ("surface", "displacement", "volume")
/// - **interface**: [`HdContainerDataSourceHandle`] - Public material parameters for UI/editing
/// - **config**: [`HdContainerDataSourceHandle`] - Renderer-specific configuration data
///
/// # USD Reference
///
/// Corresponds to `HdMaterialNetworkSchema` in OpenUSD:
/// - Header: `pxr/imaging/hd/materialNetworkSchema.h`
/// - Docs: <https://openusd.org/release/api/class_hd_material_network_schema.html>
///
/// # Example
///
/// ```ignore
/// let schema = HdMaterialNetworkSchema::new(container);
///
/// // Iterate nodes in the network
/// if let Some(nodes) = schema.get_nodes() {
///     for name in nodes.get_names() {
///         if let Some(node_data) = nodes.get(&name) {
///             // Process each shader node
///         }
///     }
/// }
///
/// // Get surface terminal connection
/// if let Some(terminals) = schema.get_terminals() {
///     if let Some(surface) = terminals.get(&Token::new("surface")) {
///         // Process surface connection
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HdMaterialNetworkSchema {
    /// Underlying schema wrapper
    schema: HdSchema,
}

impl HdMaterialNetworkSchema {
    /// Creates a new material network schema from a container data source.
    ///
    /// # Arguments
    ///
    /// * `container` - Container data source holding the network data
    ///
    /// # Returns
    ///
    /// A new `HdMaterialNetworkSchema` instance wrapping the container
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Checks if the schema container is defined (non-null).
    ///
    /// # Returns
    ///
    /// `true` if the underlying container exists, `false` otherwise
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns a reference to the underlying container data source.
    ///
    /// # Returns
    ///
    /// `Some(&HdContainerDataSourceHandle)` if defined, `None` otherwise
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Returns the container of material nodes in the network.
    ///
    /// The nodes container maps node names to [`HdMaterialNodeSchema`](super::material_node::HdMaterialNodeSchema)
    /// containers. Each node represents a shader in the material graph.
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` mapping node names to node schemas, or `None`
    pub fn get_nodes(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&NODES) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns the container of network terminals.
    ///
    /// Terminals define the outputs of the material network, typically named:
    /// - "surface" - Surface shader output
    /// - "displacement" - Displacement shader output
    /// - "volume" - Volume shader output
    ///
    /// Each terminal maps to a MaterialConnection schema pointing to a node output.
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` mapping terminal names to connections, or `None`
    pub fn get_terminals(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&TERMINALS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns the material interface schema.
    ///
    /// The interface defines public parameters exposed to users and artists for
    /// controlling material appearance. These parameters typically drive values
    /// inside the shader network.
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` containing interface parameters, or `None`
    pub fn get_interface(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&INTERFACE) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns the renderer-specific configuration container.
    ///
    /// This container holds additional settings that may be specific to
    /// particular renderers or render contexts.
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` with configuration data, or `None`
    pub fn get_config(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&CONFIG) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Builds a retained container data source for a material network.
    ///
    /// This is a convenience factory method for creating a complete material network
    /// container from individual data sources. All parameters are optional.
    ///
    /// # Arguments
    ///
    /// * `nodes` - Container mapping node names to MaterialNode schemas
    /// * `terminals` - Container mapping terminal names to output connections
    /// * `interface` - Container of public interface parameters
    /// * `config` - Renderer-specific configuration data
    ///
    /// # Returns
    ///
    /// A new `HdContainerDataSourceHandle` containing the material network data
    pub fn build_retained(
        nodes: Option<HdContainerDataSourceHandle>,
        terminals: Option<HdContainerDataSourceHandle>,
        interface: Option<HdContainerDataSourceHandle>,
        config: Option<HdContainerDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = nodes {
            entries.push((NODES.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = terminals {
            entries.push((TERMINALS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = interface {
            entries.push((INTERFACE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = config {
            entries.push((CONFIG.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for constructing material network schema data sources.
///
/// This builder provides a fluent interface for constructing a material network
/// container with optional fields. Use [`new()`](Self::new) to create a builder,
/// chain setter methods, and call [`build()`](Self::build) to produce the final container.
///
/// # Example
///
/// ```ignore
/// let network = HdMaterialNetworkSchemaBuilder::new()
///     .set_nodes(nodes_container)
///     .set_terminals(terminals_container)
///     .build();
/// ```
#[allow(dead_code)] // Ready for use when schema population is needed
#[derive(Default)]
pub struct HdMaterialNetworkSchemaBuilder {
    /// Optional nodes container
    nodes: Option<HdContainerDataSourceHandle>,
    /// Optional terminals container
    terminals: Option<HdContainerDataSourceHandle>,
    /// Optional interface container
    interface: Option<HdContainerDataSourceHandle>,
    /// Optional config container
    config: Option<HdContainerDataSourceHandle>,
}

#[allow(dead_code)]
impl HdMaterialNetworkSchemaBuilder {
    /// Creates a new empty builder.
    ///
    /// All fields are initially `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the nodes container.
    ///
    /// # Arguments
    ///
    /// * `v` - Container mapping node names to MaterialNode schemas
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_nodes(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.nodes = Some(v);
        self
    }

    /// Sets the terminals container.
    ///
    /// # Arguments
    ///
    /// * `v` - Container mapping terminal names to output connections
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_terminals(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.terminals = Some(v);
        self
    }

    /// Sets the material interface container.
    ///
    /// # Arguments
    ///
    /// * `v` - Container of public interface parameters
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_interface(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.interface = Some(v);
        self
    }

    /// Sets the renderer configuration container.
    ///
    /// # Arguments
    ///
    /// * `v` - Container of renderer-specific configuration data
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_config(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.config = Some(v);
        self
    }

    /// Builds the final material network container.
    ///
    /// Consumes the builder and returns a container data source with all
    /// configured fields.
    ///
    /// # Returns
    ///
    /// A new `HdContainerDataSourceHandle` containing the material network data
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdMaterialNetworkSchema::build_retained(
            self.nodes,
            self.terminals,
            self.interface,
            self.config,
        )
    }
}
