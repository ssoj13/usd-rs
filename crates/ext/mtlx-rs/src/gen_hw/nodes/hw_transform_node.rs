//! HwTransformPoint/Vector/Normal nodes — transform between spaces (по рефу HwTransformNode.cpp).

use crate::gen_hw::hw_constants::{block, token};
use crate::gen_shader::{
    Shader, ShaderGraph, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage,
    add_stage_uniform, shader_stage, type_desc_types,
};

const FROM_SPACE: &str = "fromspace";
const TO_SPACE: &str = "tospace";
const IN: &str = "in";
const MODEL: &str = "model";
const OBJECT: &str = "object";
const WORLD: &str = "world";

fn get_from_space(node: &ShaderNode) -> String {
    node.get_input(FROM_SPACE)
        .and_then(|i| i.port.get_value())
        .map(|v| v.get_value_string())
        .unwrap_or_default()
}

fn get_to_space(node: &ShaderNode) -> String {
    node.get_input(TO_SPACE)
        .and_then(|i| i.port.get_value())
        .map(|v| v.get_value_string())
        .unwrap_or_default()
}

fn get_matrix(from: &str, to: &str) -> Option<&'static str> {
    let from_ok = from == MODEL || from == OBJECT;
    let to_ok = to == MODEL || to == OBJECT;
    if from_ok && to == WORLD {
        Some(token::T_WORLD_MATRIX)
    } else if from == WORLD && to_ok {
        Some(token::T_WORLD_INVERSE_MATRIX)
    } else {
        None
    }
}

fn get_normal_matrix(from: &str, to: &str) -> Option<&'static str> {
    let from_ok = from == MODEL || from == OBJECT;
    let to_ok = to == MODEL || to == OBJECT;
    if from_ok && to == WORLD {
        Some(token::T_WORLD_INVERSE_TRANSPOSE_MATRIX)
    } else if from == WORLD && to_ok {
        Some(token::T_WORLD_TRANSPOSE_MATRIX)
    } else {
        None
    }
}

/// Resolve upstream "in" input to variable or default value.
fn get_upstream_in(node: &ShaderNode, graph: Option<&ShaderGraph>) -> String {
    let inp = match node.get_input(IN) {
        Some(i) => i,
        None => return "vec3(0.0, 0.0, 0.0)".to_string(),
    };
    if let Some((up_node, up_out)) = inp.get_connection() {
        if let Some(g) = graph {
            if let Some(var) = g.get_connection_variable(up_node, up_out) {
                return var;
            }
        }
    }
    let val = inp.port.get_value_string();
    if val.is_empty() {
        format!("vec3(0.0, 0.0, 0.0)")
    } else {
        format!("vec3({})", val)
    }
}

/// TransformPoint — w=1.0, uses WORLD_MATRIX / WORLD_INVERSE_MATRIX
#[derive(Debug, Default)]
pub struct HwTransformPointNode {
    name: String,
    hash: u64,
}

impl HwTransformPointNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwTransformPointNode {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_hash(&self) -> u64 {
        self.hash
    }
    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = crate::gen_shader::hash_string(&self.name);
    }
    fn create_variables(
        &self,
        node_name: &str,
        _context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        let (from, to) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            (get_from_space(node), get_to_space(node))
        };
        if let Some(matrix) = get_matrix(&from, &to) {
            if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
                add_stage_uniform(
                    block::PRIVATE_UNIFORMS,
                    type_desc_types::matrix44(),
                    matrix,
                    ps,
                );
            }
        }
    }
    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let from = get_from_space(node);
        let to = get_to_space(node);
        let matrix_opt = get_matrix(&from, &to);
        let in_val = get_upstream_in(node, context.get_graph());
        let out_var = output.port.get_variable();
        let line = if matrix_opt.is_none() {
            format!("{} = {};", out_var, in_val)
        } else {
            format!(
                "{} = mx_matrix_mul({}, vec4({}, 1.0)).xyz;",
                out_var,
                matrix_opt.unwrap(),
                in_val
            )
        };
        stage.append_line(&line);
    }
}

/// TransformVector — w=0.0, uses WORLD_MATRIX / WORLD_INVERSE_MATRIX
#[derive(Debug, Default)]
pub struct HwTransformVectorNode {
    name: String,
    hash: u64,
}

impl HwTransformVectorNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwTransformVectorNode {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_hash(&self) -> u64 {
        self.hash
    }
    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = crate::gen_shader::hash_string(&self.name);
    }
    fn create_variables(
        &self,
        node_name: &str,
        _context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        let (from, to) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            (get_from_space(node), get_to_space(node))
        };
        if let Some(matrix) = get_matrix(&from, &to) {
            if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
                add_stage_uniform(
                    block::PRIVATE_UNIFORMS,
                    type_desc_types::matrix44(),
                    matrix,
                    ps,
                );
            }
        }
    }
    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let from = get_from_space(node);
        let to = get_to_space(node);
        let matrix_opt = get_matrix(&from, &to);
        let in_val = get_upstream_in(node, context.get_graph());
        let out_var = output.port.get_variable();
        let line = if matrix_opt.is_none() {
            format!("{} = {};", out_var, in_val)
        } else {
            format!(
                "{} = mx_matrix_mul({}, vec4({}, 0.0)).xyz;",
                out_var,
                matrix_opt.unwrap(),
                in_val
            )
        };
        stage.append_line(&line);
    }
}

/// TransformNormal — w=0.0, normalize, uses WORLD_INVERSE_TRANSPOSE / WORLD_TRANSPOSE
#[derive(Debug, Default)]
pub struct HwTransformNormalNode {
    name: String,
    hash: u64,
}

impl HwTransformNormalNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwTransformNormalNode {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_hash(&self) -> u64 {
        self.hash
    }
    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = crate::gen_shader::hash_string(&self.name);
    }
    fn create_variables(
        &self,
        node_name: &str,
        _context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        let (from, to) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            (get_from_space(node), get_to_space(node))
        };
        if let Some(matrix) = get_normal_matrix(&from, &to) {
            if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
                add_stage_uniform(
                    block::PRIVATE_UNIFORMS,
                    type_desc_types::matrix44(),
                    matrix,
                    ps,
                );
            }
        }
    }
    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        // C++: validates in input type is VECTOR3 or VECTOR4
        if let Some(in_input) = node.get_input(IN) {
            let tn = in_input.port.get_type().get_name();
            if tn != "vector3" && tn != "vector4" {
                eprintln!("ERROR: Transform node must have 'in' type of vector3 or vector4.");
                return;
            }
        }
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let from = get_from_space(node);
        let to = get_to_space(node);
        let matrix_opt = get_normal_matrix(&from, &to);
        let in_val = get_upstream_in(node, context.get_graph());
        let out_var = output.port.get_variable();
        // C++: two-step emit: first assign mx_matrix_mul, then normalize separately
        if matrix_opt.is_none() {
            stage.append_line(&format!("{} = ({}).xyz;", out_var, in_val));
        } else {
            stage.append_line(&format!(
                "{} = mx_matrix_mul({}, vec4({}, 0.0)).xyz;",
                out_var,
                matrix_opt.unwrap(),
                in_val
            ));
        }
        stage.append_line(&format!("{} = normalize({});", out_var, out_var));
    }
}
