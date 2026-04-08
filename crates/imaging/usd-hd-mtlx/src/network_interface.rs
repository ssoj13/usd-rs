//! Minimal HdMaterialNetworkInterface trait for usd-hd-mtlx.
//!
//! Mirrors the relevant subset of pxr/imaging/hd/materialNetworkInterface.h
//! used by this crate. Avoids a full dependency on usd-hd.

use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Input connection from upstream node.
#[derive(Clone, Debug, Default)]
pub struct InputConnection {
    /// Name of the upstream source node.
    pub upstream_node_name: Token,
    /// Output name on the upstream node (empty = default output).
    pub upstream_output_name: Token,
}

/// Parameter data for a material node parameter.
#[derive(Clone, Debug, Default)]
pub struct NodeParamData {
    /// Parameter value.
    pub value: Value,
    /// Color space for color-valued parameters.
    pub color_space: Token,
    /// USD type name for the parameter (e.g. "color3f").
    pub type_name: Token,
}

/// Abstract interface for querying a material network.
///
/// Subset of C++ HdMaterialNetworkInterface needed by HdMtlx.
pub trait HdMaterialNetworkInterface {
    /// Path of the material prim.
    fn get_material_prim_path(&self) -> SdfPath;

    /// Get material config value by key (e.g. "mtlx:version").
    fn get_material_config_value(&self, key: &Token) -> Value;

    /// Node type (nodeIdentifier) for the given node.
    fn get_node_type(&self, node_name: &Token) -> Token;

    /// Authored parameter names for the node.
    fn get_authored_node_parameter_names(&self, node_name: &Token) -> Vec<Token>;

    /// Get full parameter data (value, colorSpace, typeName).
    fn get_node_parameter_data(&self, node_name: &Token, param_name: &Token) -> NodeParamData;

    /// Input connection names for the node.
    fn get_node_input_connection_names(&self, node_name: &Token) -> Vec<Token>;

    /// Get input connections for the given input name.
    fn get_node_input_connection(
        &self,
        node_name: &Token,
        input_name: &Token,
    ) -> Vec<InputConnection>;
}
