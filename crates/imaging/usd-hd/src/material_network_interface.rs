//! Abstract interface for querying and mutating a material network.
//!
//! Port of pxr/imaging/hd/materialNetworkInterface.h
//!
//! This is useful for implementing matfilt functions which can be reused
//! by future scene index implementations.

use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Input connection descriptor: upstream node and output.
#[derive(Clone, Debug, Default)]
pub struct InputConnection {
    /// Name of the upstream source node.
    pub upstream_node_name: Token,
    /// Output name on the upstream node.
    pub upstream_output_name: Token,
}

/// Vector of input connections (typically 1-4 per input).
pub type InputConnectionVector = Vec<InputConnection>;

/// Result of getting a terminal connection: (found, connection).
pub type InputConnectionResult = (bool, InputConnection);

/// Parameter data for a material node parameter.
#[derive(Clone, Debug, Default)]
pub struct NodeParamData {
    /// Parameter value.
    pub value: Value,
    /// Color space for color-valued parameters.
    pub color_space: Token,
    /// SdrType name for the parameter.
    pub type_name: Token,
}

/// Abstract interface for querying and mutating a material network.
///
/// Subclasses make no guarantee of thread-safety.
pub trait HdMaterialNetworkInterface: Send + Sync {
    /// Path of the material prim.
    fn get_material_prim_path(&self) -> SdfPath;

    /// Material config keys (e.g. material definition version).
    fn get_material_config_keys(&self) -> Vec<Token>;

    /// Get material config value by key.
    fn get_material_config_value(&self, key: &Token) -> Value;

    /// Nearest enclosing model asset name, or empty string.
    fn get_model_asset_name(&self) -> String;

    /// Names of nodes in the network.
    fn get_node_names(&self) -> Vec<Token>;

    /// Node type (nodeIdentifier) for the given node.
    fn get_node_type(&self, node_name: &Token) -> Token;

    /// Keys in node type info for the given node.
    fn get_node_type_info_keys(&self, node_name: &Token) -> Vec<Token>;

    /// Node type info value by key.
    fn get_node_type_info_value(&self, node_name: &Token, key: &Token) -> Value;

    /// Authored parameter names for the node.
    fn get_authored_node_parameter_names(&self, node_name: &Token) -> Vec<Token>;

    /// Get parameter value.
    fn get_node_parameter_value(&self, node_name: &Token, param_name: &Token) -> Value;

    /// Get full parameter data (value, colorSpace, typeName).
    fn get_node_parameter_data(&self, node_name: &Token, param_name: &Token) -> NodeParamData;

    /// Input connection names for the node.
    fn get_node_input_connection_names(&self, node_name: &Token) -> Vec<Token>;

    /// Get input connections for the given input.
    fn get_node_input_connection(
        &self,
        node_name: &Token,
        input_name: &Token,
    ) -> InputConnectionVector;

    /// Delete a node from the network.
    fn delete_node(&mut self, node_name: &Token);

    /// Set node type (nodeIdentifier).
    fn set_node_type(&mut self, node_name: &Token, node_type: Token);

    /// Set node type info value.
    fn set_node_type_info_value(&mut self, node_name: &Token, key: &Token, value: Value);

    /// Set node parameter value.
    fn set_node_parameter_value(&mut self, node_name: &Token, param_name: &Token, value: Value);

    /// Set full node parameter data.
    fn set_node_parameter_data(
        &mut self,
        node_name: &Token,
        param_name: &Token,
        param_data: &NodeParamData,
    );

    /// Delete a node parameter.
    fn delete_node_parameter(&mut self, node_name: &Token, param_name: &Token);

    /// Set node input connection.
    fn set_node_input_connection(
        &mut self,
        node_name: &Token,
        input_name: &Token,
        connections: &InputConnectionVector,
    );

    /// Delete node input connection.
    fn delete_node_input_connection(&mut self, node_name: &Token, input_name: &Token);

    /// Terminal names (surface, displacement, volume).
    fn get_terminal_names(&self) -> Vec<Token>;

    /// Get terminal connection. Returns (found, connection).
    fn get_terminal_connection(&self, terminal_name: &Token) -> InputConnectionResult;

    /// Delete terminal.
    fn delete_terminal(&mut self, terminal_name: &Token);

    /// Set terminal connection.
    fn set_terminal_connection(&mut self, terminal_name: &Token, connection: &InputConnection);
}
