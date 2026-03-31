
//! Legacy material network types and v2 material network structs.
//!
//! Port of pxr/imaging/hd/material.h (struct types only).
//!
//! Contains both the legacy v1 material network types (HdMaterialRelationship,
//! HdMaterialNode, HdMaterialNetwork, HdMaterialNetworkMap) and the newer v2
//! types (HdMaterialConnection2, HdMaterialNode2, HdMaterialNetwork2).
//!
//! Also provides the conversion utility `hd_convert_to_material_network2`.

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

// =============================================================================
// V1 legacy types
// =============================================================================

/// Describes a connection between two nodes in a material.
///
/// A connection consumes a value from (input_id, input_name) and passes it
/// to (output_id, output_name). Note: a connection's input is an output on the
/// upstream node, and vice versa.
#[derive(Debug, Clone)]
pub struct HdMaterialRelationship {
    /// Upstream node path (source of value).
    pub input_id: SdfPath,
    /// Upstream output name.
    pub input_name: Token,
    /// Downstream node path (consumer of value).
    pub output_id: SdfPath,
    /// Downstream input name.
    pub output_name: Token,
}

impl PartialEq for HdMaterialRelationship {
    fn eq(&self, rhs: &Self) -> bool {
        self.output_id == rhs.output_id
            && self.output_name == rhs.output_name
            && self.input_id == rhs.input_id
            && self.input_name == rhs.input_name
    }
}

impl Eq for HdMaterialRelationship {}

impl Hash for HdMaterialRelationship {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.input_id, state);
        Hash::hash(&self.input_name, state);
        Hash::hash(&self.output_id, state);
        Hash::hash(&self.output_name, state);
    }
}

/// Describes a material node: path, shader identifier, and parameters.
#[derive(Debug, Clone)]
pub struct HdMaterialNode {
    /// Node path in scene.
    pub path: SdfPath,
    /// Shader identifier (node type).
    pub identifier: Token,
    /// Parameter name -> value map.
    pub parameters: BTreeMap<Token, Value>,
}

impl PartialEq for HdMaterialNode {
    fn eq(&self, rhs: &Self) -> bool {
        self.path == rhs.path
            && self.identifier == rhs.identifier
            && self.parameters == rhs.parameters
    }
}

impl Eq for HdMaterialNode {}

impl Hash for HdMaterialNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.path, state);
        Hash::hash(&self.identifier, state);
        for (k, v) in &self.parameters {
            Hash::hash(k, state);
            Hash::hash(v, state);
        }
    }
}

/// V1 material network: nodes, relationships, and primvars.
#[derive(Debug, Clone, Default)]
pub struct HdMaterialNetworkV1 {
    /// Connections between nodes.
    pub relationships: Vec<HdMaterialRelationship>,
    /// Shader nodes.
    pub nodes: Vec<HdMaterialNode>,
    /// Primvar names used by this network.
    pub primvars: Vec<Token>,
}

impl PartialEq for HdMaterialNetworkV1 {
    fn eq(&self, rhs: &Self) -> bool {
        self.relationships == rhs.relationships
            && self.nodes == rhs.nodes
            && self.primvars == rhs.primvars
    }
}

impl Eq for HdMaterialNetworkV1 {}

impl Hash for HdMaterialNetworkV1 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.relationships, state);
        Hash::hash(&self.nodes, state);
        Hash::hash(&self.primvars, state);
    }
}

impl fmt::Display for HdMaterialNetworkV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HdMaterialNetworkV1 Params: (...)")
    }
}

/// Maps terminal name -> material network, plus terminal paths and config.
#[derive(Debug, Clone, Default)]
pub struct HdMaterialNetworkMap {
    /// Terminal name -> network mapping.
    pub map: BTreeMap<Token, HdMaterialNetworkV1>,
    /// Terminal prim paths.
    pub terminals: Vec<SdfPath>,
    /// Config dictionary.
    pub config: Dictionary,
}

impl PartialEq for HdMaterialNetworkMap {
    fn eq(&self, rhs: &Self) -> bool {
        self.map == rhs.map && self.terminals == rhs.terminals && self.config == rhs.config
    }
}

impl Eq for HdMaterialNetworkMap {}

impl Hash for HdMaterialNetworkMap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in &self.map {
            Hash::hash(k, state);
            Hash::hash(v, state);
        }
        Hash::hash(&self.terminals, state);
        Hash::hash(&self.config, state);
    }
}

/// Allow wrapping HdMaterialNetworkMap in VtValue for Hydra delegate queries.
impl From<HdMaterialNetworkMap> for Value {
    fn from(v: HdMaterialNetworkMap) -> Self {
        Value::new(v)
    }
}

impl fmt::Display for HdMaterialNetworkMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HdMaterialNetworkMap Params: (...)")
    }
}

// =============================================================================
// V2 types (replacement for v1)
// =============================================================================

/// Single connection to an upstream node and output port.
///
/// Replacement for HdMaterialRelationship.
#[derive(Debug, Clone)]
pub struct HdMaterialConnection2 {
    /// Path of the upstream node.
    pub upstream_node: SdfPath,
    /// Name of the upstream node's output.
    pub upstream_output_name: Token,
}

impl PartialEq for HdMaterialConnection2 {
    fn eq(&self, rhs: &Self) -> bool {
        self.upstream_node == rhs.upstream_node
            && self.upstream_output_name == rhs.upstream_output_name
    }
}

impl Eq for HdMaterialConnection2 {}

/// Instance of a node within a v2 material network.
///
/// Contains shader type id, parameters, and input connections.
/// A single input may have multiple upstream connections (array elements).
#[derive(Debug, Clone, Default)]
pub struct HdMaterialNode2 {
    /// Shader type identifier.
    pub node_type_id: Token,
    /// Parameter name -> value map.
    pub parameters: BTreeMap<Token, Value>,
    /// Input name -> upstream connections.
    pub input_connections: BTreeMap<Token, Vec<HdMaterialConnection2>>,
}

impl PartialEq for HdMaterialNode2 {
    fn eq(&self, rhs: &Self) -> bool {
        self.node_type_id == rhs.node_type_id
            && self.parameters == rhs.parameters
            && self.input_connections == rhs.input_connections
    }
}

impl Eq for HdMaterialNode2 {}

impl fmt::Display for HdMaterialNode2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HdMaterialNode2 Params: (...)")
    }
}

/// V2 material network: nodes keyed by path, terminal connections, primvars.
///
/// This is the mutable representation of a shading network sent to filtering
/// functions by a matfilt filter chain.
#[derive(Debug, Clone, Default)]
pub struct HdMaterialNetwork2 {
    /// Node path -> node data.
    pub nodes: BTreeMap<SdfPath, HdMaterialNode2>,
    /// Terminal name -> connection to terminal node.
    pub terminals: BTreeMap<Token, HdMaterialConnection2>,
    /// Primvar names used by this network.
    pub primvars: Vec<Token>,
    /// Config dictionary.
    pub config: Dictionary,
}

impl PartialEq for HdMaterialNetwork2 {
    fn eq(&self, rhs: &Self) -> bool {
        self.nodes == rhs.nodes
            && self.terminals == rhs.terminals
            && self.primvars == rhs.primvars
            && self.config == rhs.config
    }
}

impl Eq for HdMaterialNetwork2 {}

// =============================================================================
// Conversion: v1 -> v2
// =============================================================================

/// Token for the volume terminal name.
fn volume_token() -> Token {
    Token::new("volume")
}

/// Convert a HdMaterialNetworkMap (v1) to HdMaterialNetwork2 (v2).
///
/// Returns (network2, is_volume) where is_volume indicates whether the network
/// contained a non-empty volume terminal.
pub fn hd_convert_to_material_network2(
    hd_network_map: &HdMaterialNetworkMap,
) -> (HdMaterialNetwork2, bool) {
    let mut result = HdMaterialNetwork2::default();
    let mut is_volume = false;

    for (terminal_name, hd_network) in &hd_network_map.map {
        // Check for volume terminal
        if *terminal_name == volume_token() {
            is_volume = !hd_network.nodes.is_empty();
        }

        if hd_network.nodes.is_empty() {
            continue;
        }

        // Transfer individual nodes (may be shared across terminals).
        for node in &hd_network.nodes {
            let mat_node = result.nodes.entry(node.path.clone()).or_default();
            mat_node.node_type_id = node.identifier.clone();
            mat_node.parameters = node.parameters.clone();
        }

        // Last node is the terminal.
        if let Some(last_node) = hd_network.nodes.last() {
            result.terminals.insert(
                terminal_name.clone(),
                HdMaterialConnection2 {
                    upstream_node: last_node.path.clone(),
                    upstream_output_name: Token::default(),
                },
            );
        }

        // Transfer relationships to input connections on downstream nodes.
        for rel in &hd_network.relationships {
            // output_id is the downstream (receiving) node
            let Some(dest_node) = result.nodes.get_mut(&rel.output_id) else {
                continue;
            };

            let conn = HdMaterialConnection2 {
                upstream_node: rel.input_id.clone(),
                upstream_output_name: rel.input_name.clone(),
            };

            let conns = dest_node
                .input_connections
                .entry(rel.output_name.clone())
                .or_default();

            // Skip duplicates (may be shared between surface and displacement).
            if !conns.contains(&conn) {
                conns.push(conn);
            }
        }

        // Transfer primvars.
        result.primvars = hd_network.primvars.clone();
    }

    // Transfer config dictionary.
    result.config = hd_network_map.config.clone();

    (result, is_volume)
}

// =============================================================================
// Material-specific dirty bits (HdMaterial::DirtyBits)
// =============================================================================

/// Material-specific dirty bit constants.
///
/// These extend the base HdSprim dirty bits with material-specific flags.
pub struct HdMaterialDirtyBits;

impl HdMaterialDirtyBits {
    /// Clean state.
    pub const CLEAN: u32 = 0;
    /// Parameters changed (matches C++ DirtyParams = 1 << 2).
    pub const DIRTY_PARAMS: u32 = 1 << 2;
    /// Resource changed (matches C++ DirtyResource = 1 << 3).
    pub const DIRTY_RESOURCE: u32 = 1 << 3;
    /// Surface terminal changed.
    pub const DIRTY_SURFACE: u32 = 1 << 4;
    /// Displacement terminal changed.
    pub const DIRTY_DISPLACEMENT: u32 = 1 << 5;
    /// Volume terminal changed.
    pub const DIRTY_VOLUME: u32 = 1 << 6;
    /// All material dirty bits combined.
    pub const ALL_DIRTY: u32 = Self::DIRTY_PARAMS
        | Self::DIRTY_RESOURCE
        | Self::DIRTY_SURFACE
        | Self::DIRTY_DISPLACEMENT
        | Self::DIRTY_VOLUME;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_relationship_eq() {
        let r1 = HdMaterialRelationship {
            input_id: SdfPath::from_string("/A").unwrap(),
            input_name: Token::new("out"),
            output_id: SdfPath::from_string("/B").unwrap(),
            output_name: Token::new("in"),
        };
        let r2 = r1.clone();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_material_connection2_eq() {
        let c1 = HdMaterialConnection2 {
            upstream_node: SdfPath::from_string("/NodeA").unwrap(),
            upstream_output_name: Token::new("out"),
        };
        let c2 = c1.clone();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_material_node2_display() {
        let node = HdMaterialNode2::default();
        assert_eq!(format!("{}", node), "HdMaterialNode2 Params: (...)");
    }

    #[test]
    fn test_dirty_bits() {
        assert_eq!(HdMaterialDirtyBits::DIRTY_SURFACE, 1 << 4);
        assert_eq!(HdMaterialDirtyBits::DIRTY_DISPLACEMENT, 1 << 5);
        assert_eq!(HdMaterialDirtyBits::DIRTY_VOLUME, 1 << 6);
        assert_eq!(
            HdMaterialDirtyBits::ALL_DIRTY,
            HdMaterialDirtyBits::DIRTY_PARAMS
                | HdMaterialDirtyBits::DIRTY_RESOURCE
                | HdMaterialDirtyBits::DIRTY_SURFACE
                | HdMaterialDirtyBits::DIRTY_DISPLACEMENT
                | HdMaterialDirtyBits::DIRTY_VOLUME
        );
    }

    #[test]
    fn test_convert_empty_network_map() {
        let map = HdMaterialNetworkMap::default();
        let (net2, is_vol) = hd_convert_to_material_network2(&map);
        assert!(!is_vol);
        assert!(net2.nodes.is_empty());
        assert!(net2.terminals.is_empty());
    }

    #[test]
    fn test_convert_simple_surface() {
        let mut map = HdMaterialNetworkMap::default();

        let surface_token = Token::new("surface");
        let mut net = HdMaterialNetworkV1::default();

        let node_a = HdMaterialNode {
            path: SdfPath::from_string("/Shader").unwrap(),
            identifier: Token::new("UsdPreviewSurface"),
            parameters: BTreeMap::new(),
        };
        net.nodes.push(node_a);
        map.map.insert(surface_token.clone(), net);

        let (net2, is_vol) = hd_convert_to_material_network2(&map);
        assert!(!is_vol);
        assert_eq!(net2.nodes.len(), 1);
        assert!(net2.terminals.contains_key(&surface_token));
        let term = &net2.terminals[&surface_token];
        assert_eq!(term.upstream_node, SdfPath::from_string("/Shader").unwrap());
    }

    #[test]
    fn test_convert_with_relationships() {
        let mut map = HdMaterialNetworkMap::default();

        let surface_token = Token::new("surface");
        let mut net = HdMaterialNetworkV1::default();

        let tex_node = HdMaterialNode {
            path: SdfPath::from_string("/Tex").unwrap(),
            identifier: Token::new("UsdUVTexture"),
            parameters: BTreeMap::new(),
        };
        let surf_node = HdMaterialNode {
            path: SdfPath::from_string("/Surf").unwrap(),
            identifier: Token::new("UsdPreviewSurface"),
            parameters: BTreeMap::new(),
        };
        net.nodes.push(tex_node);
        net.nodes.push(surf_node);

        // Tex.rgb -> Surf.diffuseColor
        net.relationships.push(HdMaterialRelationship {
            input_id: SdfPath::from_string("/Tex").unwrap(),
            input_name: Token::new("rgb"),
            output_id: SdfPath::from_string("/Surf").unwrap(),
            output_name: Token::new("diffuseColor"),
        });

        map.map.insert(surface_token.clone(), net);

        let (net2, _) = hd_convert_to_material_network2(&map);
        assert_eq!(net2.nodes.len(), 2);

        // Check that Surf has inputConnections
        let surf = &net2.nodes[&SdfPath::from_string("/Surf").unwrap()];
        let dc_key = Token::new("diffuseColor");
        assert!(surf.input_connections.contains_key(&dc_key));
        let conns = &surf.input_connections[&dc_key];
        assert_eq!(conns.len(), 1);
        assert_eq!(
            conns[0].upstream_node,
            SdfPath::from_string("/Tex").unwrap()
        );
        assert_eq!(conns[0].upstream_output_name, Token::new("rgb"));
    }

    #[test]
    fn test_convert_volume_flag() {
        let mut map = HdMaterialNetworkMap::default();

        let vol_token = Token::new("volume");
        let mut net = HdMaterialNetworkV1::default();
        net.nodes.push(HdMaterialNode {
            path: SdfPath::from_string("/Vol").unwrap(),
            identifier: Token::new("VolumeShader"),
            parameters: BTreeMap::new(),
        });
        map.map.insert(vol_token, net);

        let (_, is_vol) = hd_convert_to_material_network2(&map);
        assert!(is_vol);
    }

    #[test]
    fn test_network_map_display() {
        let map = HdMaterialNetworkMap::default();
        assert_eq!(format!("{}", map), "HdMaterialNetworkMap Params: (...)");
    }
}
