//! HdMaterialNetwork2Interface - in-memory implementation of
//! HdMaterialNetworkInterface backed by HdMaterialNetwork2.
//!
//! Port of pxr/imaging/hd/materialNetwork2Interface.{h,cpp}
//!
//! Useful for implementing material filtering functions without being tied
//! to the legacy data model.

use crate::material_network::{HdMaterialConnection2, HdMaterialNetwork2, HdMaterialNode2};
use crate::material_network_interface::{
    HdMaterialNetworkInterface, InputConnection, InputConnectionResult, InputConnectionVector,
    NodeParamData,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Tokens used for parameter metadata.
fn color_space_token() -> Token {
    Token::new("colorSpace")
}

fn type_name_token() -> Token {
    Token::new("typeName")
}

/// Join two tokens with ":" separator (mirrors SdfPath::JoinIdentifier).
fn join_id(prefix: &Token, suffix: &Token) -> Token {
    Token::new(&format!("{}:{}", prefix.as_str(), suffix.as_str()))
}

/// Flatten VtDictionary keys recursively (prefix:key format).
///
/// When a value holds a nested Dictionary, recurse into it and emit
/// prefixed keys (e.g. "parent:child:leaf"). Matches C++ `_GetKeysFromVtDictionary`.
fn get_keys_from_dictionary(dict: &usd_vt::Dictionary, prefix: &str) -> Vec<Token> {
    let mut keys = Vec::new();
    for (k, v) in dict.iter() {
        let full_key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{}:{}", prefix, k)
        };
        keys.push(Token::new(&full_key));
        // Recurse into nested dictionaries
        if let Some(sub_dict) = v.get::<usd_vt::Dictionary>() {
            let sub_keys = get_keys_from_dictionary(sub_dict, &full_key);
            keys.extend(sub_keys);
        }
    }
    keys
}

/// Implementation of HdMaterialNetworkInterface backed by HdMaterialNetwork2.
///
/// All reads and writes go directly to the in-memory HdMaterialNetwork2 struct.
pub struct HdMaterialNetwork2Interface {
    material_prim_path: SdfPath,
    material_network: HdMaterialNetwork2,
    /// Cached last-accessed node name for fast repeated lookups.
    _last_accessed_name: Option<SdfPath>,
}

impl HdMaterialNetwork2Interface {
    /// Create interface wrapping a material network.
    pub fn new(material_prim_path: SdfPath, material_network: HdMaterialNetwork2) -> Self {
        Self {
            material_prim_path,
            material_network,
            _last_accessed_name: None,
        }
    }

    /// Borrow the underlying network.
    pub fn network(&self) -> &HdMaterialNetwork2 {
        &self.material_network
    }

    /// Consume and return the underlying network.
    pub fn into_network(self) -> HdMaterialNetwork2 {
        self.material_network
    }

    /// Lookup node by token name (converted to SdfPath).
    fn get_node(&self, node_name: &Token) -> Option<&HdMaterialNode2> {
        let path = SdfPath::from_string(node_name.as_str());
        match path {
            Some(p) => self.material_network.nodes.get(&p),
            None => None,
        }
    }

    /// Lookup mutable node, creating it if absent.
    fn get_or_create_node(&mut self, node_name: &Token) -> &mut HdMaterialNode2 {
        let path = SdfPath::from_string(node_name.as_str())
            .unwrap_or_else(|| SdfPath::from_string("/").unwrap());
        self.material_network.nodes.entry(path).or_default()
    }

    /// Lookup mutable node (no create).
    fn get_node_mut(&mut self, node_name: &Token) -> Option<&mut HdMaterialNode2> {
        let path = SdfPath::from_string(node_name.as_str());
        match path {
            Some(p) => self.material_network.nodes.get_mut(&p),
            None => None,
        }
    }
}

impl HdMaterialNetworkInterface for HdMaterialNetwork2Interface {
    fn get_material_prim_path(&self) -> SdfPath {
        self.material_prim_path.clone()
    }

    fn get_material_config_keys(&self) -> Vec<Token> {
        get_keys_from_dictionary(&self.material_network.config, "")
    }

    fn get_material_config_value(&self, key: &Token) -> Value {
        self.material_network
            .config
            .get(key.as_str())
            .cloned()
            .unwrap_or_else(|| Value::empty())
    }

    fn get_model_asset_name(&self) -> String {
        String::new()
    }

    fn get_node_names(&self) -> Vec<Token> {
        self.material_network
            .nodes
            .keys()
            .map(|p| Token::new(p.as_str()))
            .collect()
    }

    fn get_node_type(&self, node_name: &Token) -> Token {
        self.get_node(node_name)
            .map(|n| n.node_type_id.clone())
            .unwrap_or_default()
    }

    fn get_node_type_info_keys(&self, _node_name: &Token) -> Vec<Token> {
        // No-op: HdMaterialNetwork2 doesn't store node type info.
        Vec::new()
    }

    fn get_node_type_info_value(&self, _node_name: &Token, _key: &Token) -> Value {
        Value::empty()
    }

    fn get_authored_node_parameter_names(&self, node_name: &Token) -> Vec<Token> {
        self.get_node(node_name)
            .map(|n| n.parameters.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn get_node_parameter_value(&self, node_name: &Token, param_name: &Token) -> Value {
        self.get_node(node_name)
            .and_then(|n| n.parameters.get(param_name))
            .cloned()
            .unwrap_or_else(Value::empty)
    }

    fn get_node_parameter_data(&self, node_name: &Token, param_name: &Token) -> NodeParamData {
        let mut data = NodeParamData::default();
        if let Some(node) = self.get_node(node_name) {
            // Value
            if let Some(v) = node.parameters.get(param_name) {
                data.value = v.clone();
            }
            // ColorSpace
            let cs_key = join_id(&color_space_token(), param_name);
            if let Some(v) = node.parameters.get(&cs_key) {
                if let Some(t) = v.get::<Token>() {
                    data.color_space = t.clone();
                }
            }
            // TypeName
            let tn_key = join_id(&type_name_token(), param_name);
            if let Some(v) = node.parameters.get(&tn_key) {
                if let Some(t) = v.get::<Token>() {
                    data.type_name = t.clone();
                }
            }
        }
        data
    }

    fn get_node_input_connection_names(&self, node_name: &Token) -> Vec<Token> {
        self.get_node(node_name)
            .map(|n| n.input_connections.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn get_node_input_connection(
        &self,
        node_name: &Token,
        input_name: &Token,
    ) -> InputConnectionVector {
        self.get_node(node_name)
            .and_then(|n| n.input_connections.get(input_name))
            .map(|conns| {
                conns
                    .iter()
                    .map(|c| InputConnection {
                        upstream_node_name: Token::new(c.upstream_node.as_str()),
                        upstream_output_name: c.upstream_output_name.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn delete_node(&mut self, node_name: &Token) {
        if let Some(p) = SdfPath::from_string(node_name.as_str()) {
            self.material_network.nodes.remove(&p);
        }
    }

    fn set_node_type(&mut self, node_name: &Token, node_type: Token) {
        let node = self.get_or_create_node(node_name);
        node.node_type_id = node_type;
    }

    fn set_node_type_info_value(&mut self, _node_name: &Token, _key: &Token, _value: Value) {
        // No-op: HdMaterialNetwork2 doesn't store node type info.
    }

    fn set_node_parameter_value(&mut self, node_name: &Token, param_name: &Token, value: Value) {
        let node = self.get_or_create_node(node_name);
        node.parameters.insert(param_name.clone(), value);
    }

    fn set_node_parameter_data(
        &mut self,
        node_name: &Token,
        param_name: &Token,
        param_data: &NodeParamData,
    ) {
        let node = self.get_or_create_node(node_name);
        // Value
        node.parameters
            .insert(param_name.clone(), param_data.value.clone());
        // ColorSpace
        if !param_data.color_space.is_empty() {
            let cs_key = join_id(&color_space_token(), param_name);
            node.parameters
                .insert(cs_key, Value::from(param_data.color_space.clone()));
        }
        // TypeName
        if !param_data.type_name.is_empty() {
            let tn_key = join_id(&type_name_token(), param_name);
            node.parameters
                .insert(tn_key, Value::from(param_data.type_name.clone()));
        }
    }

    fn delete_node_parameter(&mut self, node_name: &Token, param_name: &Token) {
        if let Some(node) = self.get_node_mut(node_name) {
            node.parameters.remove(param_name);
            // Also remove colorSpace and typeName metadata.
            let cs_key = join_id(&color_space_token(), param_name);
            node.parameters.remove(&cs_key);
            let tn_key = join_id(&type_name_token(), param_name);
            node.parameters.remove(&tn_key);
        }
    }

    fn set_node_input_connection(
        &mut self,
        node_name: &Token,
        input_name: &Token,
        connections: &InputConnectionVector,
    ) {
        let node = self.get_or_create_node(node_name);
        let conns2: Vec<HdMaterialConnection2> = connections
            .iter()
            .map(|c| HdMaterialConnection2 {
                upstream_node: SdfPath::from_string(c.upstream_node_name.as_str())
                    .unwrap_or_else(|| SdfPath::from_string("/").unwrap()),
                upstream_output_name: c.upstream_output_name.clone(),
            })
            .collect();
        node.input_connections.insert(input_name.clone(), conns2);
    }

    fn delete_node_input_connection(&mut self, node_name: &Token, input_name: &Token) {
        if let Some(node) = self.get_node_mut(node_name) {
            node.input_connections.remove(input_name);
        }
    }

    fn get_terminal_names(&self) -> Vec<Token> {
        self.material_network.terminals.keys().cloned().collect()
    }

    fn get_terminal_connection(&self, terminal_name: &Token) -> InputConnectionResult {
        match self.material_network.terminals.get(terminal_name) {
            Some(c) => (
                true,
                InputConnection {
                    upstream_node_name: Token::new(c.upstream_node.as_str()),
                    upstream_output_name: c.upstream_output_name.clone(),
                },
            ),
            None => (false, InputConnection::default()),
        }
    }

    fn delete_terminal(&mut self, terminal_name: &Token) {
        self.material_network.terminals.remove(terminal_name);
    }

    fn set_terminal_connection(&mut self, terminal_name: &Token, connection: &InputConnection) {
        self.material_network.terminals.insert(
            terminal_name.clone(),
            HdMaterialConnection2 {
                upstream_node: SdfPath::from_string(connection.upstream_node_name.as_str())
                    .unwrap_or_else(|| SdfPath::from_string("/").unwrap()),
                upstream_output_name: connection.upstream_output_name.clone(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material_network::HdMaterialNetwork2;

    fn make_test_network() -> HdMaterialNetwork2 {
        let mut net = HdMaterialNetwork2::default();
        let node_path = SdfPath::from_string("/Shader").unwrap();
        let mut node = HdMaterialNode2::default();
        node.node_type_id = Token::new("UsdPreviewSurface");
        node.parameters
            .insert(Token::new("roughness"), Value::from(0.5f32));
        net.nodes.insert(node_path.clone(), node);
        net.terminals.insert(
            Token::new("surface"),
            HdMaterialConnection2 {
                upstream_node: node_path,
                upstream_output_name: Token::default(),
            },
        );
        net
    }

    #[test]
    fn test_get_node_names() {
        let net = make_test_network();
        let iface = HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);
        let names = iface.get_node_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].as_str(), "/Shader");
    }

    #[test]
    fn test_get_node_type() {
        let net = make_test_network();
        let iface = HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);
        let t = iface.get_node_type(&Token::new("/Shader"));
        assert_eq!(t.as_str(), "UsdPreviewSurface");
    }

    #[test]
    fn test_get_param_value() {
        let net = make_test_network();
        let iface = HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);
        let v = iface.get_node_parameter_value(&Token::new("/Shader"), &Token::new("roughness"));
        assert_eq!(v.get::<f32>(), Some(&0.5f32));
    }

    #[test]
    fn test_set_and_delete_param() {
        let net = make_test_network();
        let mut iface =
            HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);

        let node = Token::new("/Shader");
        let param = Token::new("metallic");

        iface.set_node_parameter_value(&node, &param, Value::from(1.0f32));
        let v = iface.get_node_parameter_value(&node, &param);
        assert_eq!(v.get::<f32>(), Some(&1.0f32));

        iface.delete_node_parameter(&node, &param);
        let v = iface.get_node_parameter_value(&node, &param);
        assert!(v.is_empty());
    }

    #[test]
    fn test_terminal_ops() {
        let net = make_test_network();
        let mut iface =
            HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);

        let names = iface.get_terminal_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].as_str(), "surface");

        let (found, conn) = iface.get_terminal_connection(&Token::new("surface"));
        assert!(found);
        assert_eq!(conn.upstream_node_name.as_str(), "/Shader");

        iface.delete_terminal(&Token::new("surface"));
        let names = iface.get_terminal_names();
        assert!(names.is_empty());
    }

    #[test]
    fn test_set_terminal_connection() {
        let net = HdMaterialNetwork2::default();
        let mut iface =
            HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);

        iface.set_terminal_connection(
            &Token::new("displacement"),
            &InputConnection {
                upstream_node_name: Token::new("/DispShader"),
                upstream_output_name: Token::new("out"),
            },
        );

        let (found, conn) = iface.get_terminal_connection(&Token::new("displacement"));
        assert!(found);
        assert_eq!(conn.upstream_node_name.as_str(), "/DispShader");
        assert_eq!(conn.upstream_output_name.as_str(), "out");
    }

    #[test]
    fn test_input_connections() {
        let mut net = HdMaterialNetwork2::default();
        let tex_path = SdfPath::from_string("/Tex").unwrap();
        let surf_path = SdfPath::from_string("/Surf").unwrap();

        let tex_node = HdMaterialNode2 {
            node_type_id: Token::new("UsdUVTexture"),
            ..Default::default()
        };
        let mut surf_node = HdMaterialNode2 {
            node_type_id: Token::new("UsdPreviewSurface"),
            ..Default::default()
        };
        surf_node.input_connections.insert(
            Token::new("diffuseColor"),
            vec![HdMaterialConnection2 {
                upstream_node: tex_path.clone(),
                upstream_output_name: Token::new("rgb"),
            }],
        );

        net.nodes.insert(tex_path, tex_node);
        net.nodes.insert(surf_path, surf_node);

        let iface = HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);

        let conn_names = iface.get_node_input_connection_names(&Token::new("/Surf"));
        assert_eq!(conn_names.len(), 1);
        assert_eq!(conn_names[0].as_str(), "diffuseColor");

        let conns =
            iface.get_node_input_connection(&Token::new("/Surf"), &Token::new("diffuseColor"));
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].upstream_node_name.as_str(), "/Tex");
        assert_eq!(conns[0].upstream_output_name.as_str(), "rgb");
    }

    #[test]
    fn test_delete_node() {
        let net = make_test_network();
        let mut iface =
            HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);
        assert_eq!(iface.get_node_names().len(), 1);

        iface.delete_node(&Token::new("/Shader"));
        assert!(iface.get_node_names().is_empty());
    }

    #[test]
    fn test_set_node_type() {
        let net = HdMaterialNetwork2::default();
        let mut iface =
            HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);

        iface.set_node_type(&Token::new("/NewNode"), Token::new("MyShader"));
        let t = iface.get_node_type(&Token::new("/NewNode"));
        assert_eq!(t.as_str(), "MyShader");
    }

    #[test]
    fn test_model_asset_name_empty() {
        let net = HdMaterialNetwork2::default();
        let iface = HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);
        assert!(iface.get_model_asset_name().is_empty());
    }

    #[test]
    fn test_into_network() {
        let net = make_test_network();
        let iface = HdMaterialNetwork2Interface::new(SdfPath::from_string("/Mat").unwrap(), net);
        let recovered = iface.into_network();
        assert_eq!(recovered.nodes.len(), 1);
    }
}
