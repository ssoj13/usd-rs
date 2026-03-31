//! Material node schema for Hydra.
//!
//! This module provides the [`HdMaterialNodeSchema`] which represents a single shader node
//! in a material network. Each node contains parameters, input connections, and shader
//! identifier information.
//!
//! # Overview
//!
//! A material node defines a shader in the rendering pipeline with:
//! - **Parameters**: Direct value inputs to the shader (constants)
//! - **Input connections**: Links to outputs from other nodes in the network
//! - **Node identifier**: Shader type name (e.g., "UsdPreviewSurface")
//! - **Render context identifiers**: Alternative shader names per render context
//! - **Type info**: Additional metadata about the shader type
//!
//! # USD Reference
//!
//! This corresponds to OpenUSD's `HdMaterialNodeSchema`:
//! - USD: `pxr/imaging/hd/materialNodeSchema.h`
//! - Docs: <https://openusd.org/release/api/class_hd_material_node_schema.html>
//!
//! # Example
//!
//! ```ignore
//! use usd_hd::schema::material_node::*;
//!
//! // Create a material node using the builder
//! let node = HdMaterialNodeSchemaBuilder::new()
//!     .set_node_identifier(token_data_source)
//!     .set_parameters(params_container)
//!     .set_input_connections(connections_container)
//!     .build();
//!
//! // Access the node schema
//! let schema = HdMaterialNodeSchema::new(node);
//! if let Some(identifier) = schema.get_node_identifier() {
//!     // Use the shader identifier
//! }
//! ```

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Schema locator token for node parameters container.
///
/// This contains the direct value inputs to the shader node.
pub static PARAMETERS: Lazy<Token> = Lazy::new(|| Token::new("parameters"));

/// Schema locator token for input connections container.
///
/// This maps input names to connection descriptors pointing to upstream nodes.
pub static INPUT_CONNECTIONS: Lazy<Token> = Lazy::new(|| Token::new("inputConnections"));

/// Schema locator token for node identifier.
///
/// The shader type identifier (e.g., "UsdPreviewSurface", "StandardSurface").
pub static NODE_IDENTIFIER: Lazy<Token> = Lazy::new(|| Token::new("nodeIdentifier"));

/// Schema locator token for render context-specific node identifiers.
///
/// Container mapping render context names to alternative shader identifiers.
pub static RENDER_CONTEXT_NODE_IDENTIFIERS: Lazy<Token> =
    Lazy::new(|| Token::new("renderContextNodeIdentifiers"));

/// Schema locator token for node type information.
///
/// Additional metadata about the shader node type.
pub static NODE_TYPE_INFO: Lazy<Token> = Lazy::new(|| Token::new("nodeTypeInfo"));

/// Trait object for typed data source holding a Token value.
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;

/// Reference-counted handle to a Token data source.
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema representing a material node in Hydra's material system.
///
/// This schema wraps a container data source containing the shader node definition.
/// A material node represents a single shader in the material network graph.
///
/// # Schema Fields
///
/// - **parameters**: [`HdContainerDataSourceHandle`] - Direct value inputs to the shader
/// - **inputConnections**: [`HdContainerDataSourceHandle`] - Maps input names to connection vectors
/// - **nodeIdentifier**: [`HdTokenDataSourceHandle`] - Shader type (e.g., "UsdPreviewSurface")
/// - **renderContextNodeIdentifiers**: [`HdContainerDataSourceHandle`] - Alternative shader names per context
/// - **nodeTypeInfo**: [`HdContainerDataSourceHandle`] - Additional shader metadata
///
/// # USD Reference
///
/// Corresponds to `HdMaterialNodeSchema` in OpenUSD:
/// - Header: `pxr/imaging/hd/materialNodeSchema.h`
/// - Docs: <https://openusd.org/release/api/class_hd_material_node_schema.html>
///
/// # Example
///
/// ```ignore
/// let schema = HdMaterialNodeSchema::new(container);
/// if let Some(params) = schema.get_parameters() {
///     // Process node parameters
/// }
/// if let Some(id) = schema.get_node_identifier() {
///     // Get shader identifier
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HdMaterialNodeSchema {
    /// Underlying schema wrapper
    schema: HdSchema,
}

impl HdMaterialNodeSchema {
    /// Creates a new material node schema from a container data source.
    ///
    /// # Arguments
    ///
    /// * `container` - Container data source holding the node data
    ///
    /// # Returns
    ///
    /// A new `HdMaterialNodeSchema` instance wrapping the container
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

    /// Returns the container of node parameters (direct value inputs).
    ///
    /// Parameters are constant values provided directly to the shader,
    /// as opposed to values computed by upstream nodes via connections.
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` containing the parameters, or `None`
    pub fn get_parameters(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&PARAMETERS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns the container of input connections.
    ///
    /// Input connections define the material network topology by linking
    /// this node's inputs to outputs from upstream nodes in the graph.
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` mapping input names to connection vectors, or `None`
    pub fn get_input_connections(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&INPUT_CONNECTIONS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns the shader node identifier.
    ///
    /// The node identifier is a token specifying the shader type,
    /// such as "UsdPreviewSurface", "StandardSurface", etc.
    ///
    /// # Returns
    ///
    /// `Some(HdTokenDataSourceHandle)` containing the identifier, or `None`
    pub fn get_node_identifier(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&NODE_IDENTIFIER)
    }

    /// Returns render-context-specific node identifiers.
    ///
    /// This container maps render context names to alternative shader identifiers,
    /// allowing different shaders to be used in different rendering backends
    /// (e.g., "storm" vs "karma").
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` mapping context names to identifiers, or `None`
    pub fn get_render_context_node_identifiers(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&RENDER_CONTEXT_NODE_IDENTIFIERS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns additional node type information.
    ///
    /// This container holds extra metadata about the shader node type,
    /// which may be used for shader discovery and validation.
    ///
    /// # Returns
    ///
    /// `Some(HdContainerDataSourceHandle)` with type metadata, or `None`
    pub fn get_node_type_info(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&NODE_TYPE_INFO) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Builds a retained container data source for a material node.
    ///
    /// This is a convenience factory method for creating a complete material node
    /// container from individual data sources. All parameters are optional.
    ///
    /// # Arguments
    ///
    /// * `parameters` - Container of node parameters (direct value inputs)
    /// * `input_connections` - Container of input connections to upstream nodes
    /// * `node_identifier` - Shader type identifier token
    /// * `render_context_node_identifiers` - Context-specific shader identifiers
    /// * `node_type_info` - Additional shader type metadata
    ///
    /// # Returns
    ///
    /// A new `HdContainerDataSourceHandle` containing the material node data
    pub fn build_retained(
        parameters: Option<HdContainerDataSourceHandle>,
        input_connections: Option<HdContainerDataSourceHandle>,
        node_identifier: Option<HdTokenDataSourceHandle>,
        render_context_node_identifiers: Option<HdContainerDataSourceHandle>,
        node_type_info: Option<HdContainerDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = parameters {
            entries.push((PARAMETERS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = input_connections {
            entries.push((INPUT_CONNECTIONS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = node_identifier {
            entries.push((NODE_IDENTIFIER.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = render_context_node_identifiers {
            entries.push((
                RENDER_CONTEXT_NODE_IDENTIFIERS.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = node_type_info {
            entries.push((NODE_TYPE_INFO.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for constructing material node schema data sources.
///
/// This builder provides a fluent interface for constructing a material node
/// container with optional fields. Use [`new()`](Self::new) to create a builder,
/// chain setter methods, and call [`build()`](Self::build) to produce the final container.
///
/// # Example
///
/// ```ignore
/// let node = HdMaterialNodeSchemaBuilder::new()
///     .set_node_identifier(token_ds)
///     .set_parameters(params_ds)
///     .build();
/// ```
#[allow(dead_code)] // Ready for use when schema population is needed
#[derive(Default)]
pub struct HdMaterialNodeSchemaBuilder {
    /// Optional parameters container
    parameters: Option<HdContainerDataSourceHandle>,
    /// Optional input connections container
    input_connections: Option<HdContainerDataSourceHandle>,
    /// Optional node identifier
    node_identifier: Option<HdTokenDataSourceHandle>,
    /// Optional render context identifiers container
    render_context_node_identifiers: Option<HdContainerDataSourceHandle>,
    /// Optional node type info container
    node_type_info: Option<HdContainerDataSourceHandle>,
}

#[allow(dead_code)]
impl HdMaterialNodeSchemaBuilder {
    /// Creates a new empty builder.
    ///
    /// All fields are initially `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the parameters container (direct value inputs).
    ///
    /// # Arguments
    ///
    /// * `v` - Container of parameter data sources
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_parameters(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.parameters = Some(v);
        self
    }

    /// Sets the input connections container.
    ///
    /// # Arguments
    ///
    /// * `v` - Container mapping input names to connection vectors
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_input_connections(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.input_connections = Some(v);
        self
    }

    /// Sets the shader node identifier.
    ///
    /// # Arguments
    ///
    /// * `v` - Token data source containing the shader type identifier
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_node_identifier(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.node_identifier = Some(v);
        self
    }

    /// Sets the render context-specific node identifiers.
    ///
    /// # Arguments
    ///
    /// * `v` - Container mapping context names to shader identifiers
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_render_context_node_identifiers(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.render_context_node_identifiers = Some(v);
        self
    }

    /// Sets additional node type information.
    ///
    /// # Arguments
    ///
    /// * `v` - Container of shader type metadata
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_node_type_info(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.node_type_info = Some(v);
        self
    }

    /// Builds the final material node container.
    ///
    /// Consumes the builder and returns a container data source with all
    /// configured fields.
    ///
    /// # Returns
    ///
    /// A new `HdContainerDataSourceHandle` containing the material node data
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdMaterialNodeSchema::build_retained(
            self.parameters,
            self.input_connections,
            self.node_identifier,
            self.render_context_node_identifiers,
            self.node_type_info,
        )
    }
}
