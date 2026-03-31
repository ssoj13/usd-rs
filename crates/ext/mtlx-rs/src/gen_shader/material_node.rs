//! MaterialNode — material node implementation (по рефу MaterialX MaterialNode).

use super::gen_context::ShaderImplContext;
use super::shader::ShaderStage;
use super::shader_node::{ShaderNode, ShaderNodeClassification};
use super::shader_node_impl::ShaderNodeImpl;

const SURFACESHADER: &str = "surfaceshader";

/// Material node implementation.
#[derive(Debug, Default)]
pub struct MaterialNode {
    name: String,
    hash: u64,
}

impl MaterialNode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    fn resolve_surfaceshader_rhs(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
    ) -> String {
        let inp = match node.get_input(SURFACESHADER) {
            Some(i) => i,
            None => return "vec4(0.0, 0.0, 0.0, 1.0)".to_string(),
        };
        if let Some((up_node, up_output)) = inp.get_connection() {
            if let Some(g) = context.get_graph() {
                if let Some(var) = g.get_connection_variable(up_node, up_output) {
                    return var;
                }
            }
        }
        "vec4(0.0, 0.0, 0.0, 1.0)".to_string()
    }
}

fn glsl_type_name(mtlx: &str) -> &'static str {
    match mtlx {
        "float" => "float",
        "integer" => "int",
        "boolean" => "bool",
        "vector2" => "vec2",
        "vector3" => "vec3",
        "vector4" => "vec4",
        "color3" => "vec3",
        "color4" => "vec4",
        "matrix33" => "mat3",
        "matrix44" => "mat4",
        "surfaceshader" | "material" => "vec4",
        _ => "float",
    }
}

impl ShaderNodeImpl for MaterialNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = super::util::hash_string(&self.name);
    }

    /// Add classification from connected surfaceshader node upstream.
    /// Matches C++ MaterialNode::addClassification: propagates shader classification.
    fn add_classification(&self, node: &mut ShaderNode) {
        // If a surfaceshader input exists and is connected, propagate the
        // upstream node's classification to this material node (по рефу MaterialNode.cpp ~18).
        if let Some(inp) = node.get_input(SURFACESHADER) {
            if inp.has_connection() {
                // The upstream classification will be applied during graph finalize
                // when the graph resolves classification from the output socket's upstream.
                // Mark this node as material.
                node.add_classification(ShaderNodeClassification::MATERIAL);
            }
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != super::shader::stage::PIXEL {
            return;
        }
        self.emit_output_variables(node, context, stage);
        let rhs = self.resolve_surfaceshader_rhs(node, context);
        let output = node.get_outputs().next().expect("Material has out");
        let out_var = output.port.get_variable();
        stage.append_line(&format!("{} = {};", out_var, rhs));
    }

    fn emit_output_variables(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let type_system = context.get_type_system();
        for output in node.get_outputs() {
            let type_desc = output.get_type();
            let mtlx_type = type_system.get_type(type_desc.get_name());
            let type_name = glsl_type_name(mtlx_type.get_name());
            let var = output.port.get_variable();
            let default_val = "vec4(0.0)";
            stage.append_line(&format!("{} {} = {};", type_name, var, default_val));
        }
    }
}
