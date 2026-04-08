//! Render terminal helper for RenderMan-style prims.
//!
//! Port of `pxr/usdImaging/usdRiPxrImaging/pxrRenderTerminalHelper.cpp`.

use std::collections::HashMap;

use usd_core::Prim;
use usd_tf::Token;
use usd_vt::{TimeCode, Value};

/// Represents a material node for Hydra.
#[derive(Debug, Clone, Default)]
pub struct HdMaterialNode2 {
    pub node_type_id: Token,
    pub parameters: HashMap<Token, Value>,
    pub input_connections: HashMap<Token, Vec<HdMaterialConnection2>>,
}

/// Represents a connection between material nodes.
#[derive(Debug, Clone)]
pub struct HdMaterialConnection2 {
    pub upstream_node: usd_sdf::Path,
    pub upstream_output_name: Token,
}

pub struct RenderTerminalHelper;

impl RenderTerminalHelper {
    fn node_type_id(prim: &Prim, shader_id_token: &Token, prim_type_token: &Token) -> Token {
        let Some(attr) = prim.get_attribute(shader_id_token.as_str()) else {
            return prim_type_token.clone();
        };
        let Some(value) = attr.get(TimeCode::default()) else {
            return prim_type_token.clone();
        };
        value
            .downcast_clone::<Token>()
            .or_else(|| value.downcast_clone::<String>().map(|s| Token::new(&s)))
            .unwrap_or_else(|| prim_type_token.clone())
    }

    pub fn create_hd_material_node2(
        prim: &Prim,
        shader_id_token: &Token,
        prim_type_token: &Token,
    ) -> HdMaterialNode2 {
        let mut parameters = HashMap::new();
        for attr in prim.get_attributes() {
            let name = attr.name();
            let Some(input_name) = Self::strip_input_prefix(name.as_str()) else {
                continue;
            };
            if !attr.has_authored_value() {
                continue;
            }
            let Some(value) = attr.get(TimeCode::default()) else {
                continue;
            };
            parameters.insert(Token::new(input_name), value);
        }

        HdMaterialNode2 {
            node_type_id: Self::node_type_id(prim, shader_id_token, prim_type_token),
            parameters,
            input_connections: HashMap::new(),
        }
    }

    pub fn has_input_prefix(attr_name: &str) -> bool {
        attr_name.starts_with("inputs:")
    }

    pub fn strip_input_prefix(attr_name: &str) -> Option<&str> {
        attr_name.strip_prefix("inputs:")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;
    use usd_core::common::InitialLoadSet;

    #[test]
    fn test_has_input_prefix() {
        assert!(RenderTerminalHelper::has_input_prefix("inputs:maxSamples"));
        assert!(!RenderTerminalHelper::has_input_prefix("ri:integratorType"));
    }

    #[test]
    fn test_strip_input_prefix() {
        assert_eq!(
            RenderTerminalHelper::strip_input_prefix("inputs:maxSamples"),
            Some("maxSamples")
        );
        assert_eq!(
            RenderTerminalHelper::strip_input_prefix("ri:integratorType"),
            None
        );
    }

    #[test]
    fn test_create_node_falls_back_to_prim_type() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let node = RenderTerminalHelper::create_hd_material_node2(
            &stage.get_pseudo_root(),
            &Token::new("ri:integratorType"),
            &Token::new("PxrPathTracer"),
        );
        assert_eq!(node.node_type_id.as_str(), "PxrPathTracer");
    }
}
