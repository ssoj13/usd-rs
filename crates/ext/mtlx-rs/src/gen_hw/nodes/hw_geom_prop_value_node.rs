//! HwGeomPropValueNode — geometry property value (по рефу HwGeomPropValueNode.cpp).

use crate::core::GEOM_PROP_ATTRIBUTE;
use crate::gen_hw::hw_constants::block;
use crate::gen_hw::hw_constants::token;
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_input,
    add_stage_output, add_stage_uniform, shader_stage,
};

/// GeomPropValue from vertex stage (vertex varying).
#[derive(Debug, Default)]
pub struct HwGeomPropValueNode {
    name: String,
    hash: u64,
}

impl HwGeomPropValueNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }

    fn get_geom_prop(node: &ShaderNode) -> Option<String> {
        node.get_input(GEOM_PROP_ATTRIBUTE)
            .and_then(|i| i.port.get_value())
            .map(|v| v.get_value_string())
    }
}

impl ShaderNodeImpl for HwGeomPropValueNode {
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

    fn is_editable(&self, _input_name: &str) -> bool {
        false
    }

    fn create_variables(
        &self,
        node_name: &str,
        _context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        let (_geom_prop, type_desc, in_name) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            // C++: throws ExceptionShaderGenError if geomprop input is missing
            let geom_prop = match Self::get_geom_prop(node) {
                Some(g) if !g.is_empty() => g,
                _ => {
                    eprintln!(
                        "ERROR: No 'geomprop' parameter found on geompropvalue node '{}'",
                        node_name
                    );
                    return;
                }
            };
            let output = match node.get_outputs().next() {
                Some(o) => o,
                None => return,
            };
            let type_desc = output.get_type().clone();
            let in_name = format!("{}_{}", token::T_IN_GEOMPROP, geom_prop);
            (geom_prop, type_desc, in_name)
        };

        if let Some(vs) = shader.get_stage_by_name_mut(shader_stage::VERTEX) {
            add_stage_input(block::VERTEX_INPUTS, type_desc.clone(), &in_name, vs, false);
            add_stage_output(block::VERTEX_DATA, type_desc.clone(), &in_name, vs, false);
        }
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            add_stage_input(block::VERTEX_DATA, type_desc, &in_name, ps, false);
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let geom_prop = match Self::get_geom_prop(node) {
            Some(g) if !g.is_empty() => g,
            _ => return,
        };
        let variable = format!("{}_{}", token::T_IN_GEOMPROP, geom_prop);
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };

        if stage.get_name() == shader_stage::VERTEX {
            let (line, need_emit) = {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(geomport) = vertex_data.find_mut(&variable) {
                    if !geomport.is_emitted() {
                        geomport.set_emitted(true);
                        let l = format!(
                            "vd.{} = {}_{};",
                            geomport.get_variable(),
                            token::T_IN_GEOMPROP,
                            geom_prop
                        );
                        (l, true)
                    } else {
                        (String::new(), false)
                    }
                } else {
                    (String::new(), false)
                }
            };
            if need_emit {
                stage.append_line(&line);
            }
        } else if stage.get_name() == shader_stage::PIXEL {
            let vertex_data = match stage.get_input_block(block::VERTEX_DATA) {
                Some(b) => b,
                None => return,
            };
            if let Some(geomport) = vertex_data.find(&variable) {
                // C++ emitOutput(node.getOutput(), true, false, ...) — includeDeclaration=true
                let type_desc = output.get_type();
                let type_name =
                    crate::gen_shader::source_code_node::glsl_type_name(type_desc.get_name());
                let line = format!(
                    "{} {} = vd.{};",
                    type_name,
                    output.port.get_variable(),
                    geomport.get_variable()
                );
                stage.append_line(&line);
            }
        }
    }
}

/// GeomPropValue as uniform (for boolean, string, filename - no vertex interpolation).
#[derive(Debug, Default)]
pub struct HwGeomPropValueNodeAsUniform {
    name: String,
    hash: u64,
}

impl HwGeomPropValueNodeAsUniform {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwGeomPropValueNodeAsUniform {
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
        let (_geom_prop, type_desc, uniform_name, geom_path) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            // C++: throws ExceptionShaderGenError if geomprop input is missing
            let geom_prop = match HwGeomPropValueNode::get_geom_prop(node) {
                Some(g) if !g.is_empty() => g,
                _ => {
                    eprintln!(
                        "ERROR: No 'geomprop' parameter found on geompropvalue node '{}'",
                        node_name
                    );
                    return;
                }
            };
            let output = match node.get_outputs().next() {
                Some(o) => o,
                None => return,
            };
            let type_desc = output.get_type().clone();
            let uniform_name = format!("{}_{}", token::T_GEOMPROP, geom_prop);
            // C++: uniform->setPath(geomPropInput->getPath())
            let path = node
                .get_input(GEOM_PROP_ATTRIBUTE)
                .map(|i| i.port.path.clone())
                .unwrap_or_default();
            (geom_prop, type_desc, uniform_name, path)
        };

        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            let port = add_stage_uniform(block::PRIVATE_UNIFORMS, type_desc, &uniform_name, ps);
            // C++: uniform->setPath(geomPropInput->getPath())
            port.path = geom_path;
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        let geom_prop = match HwGeomPropValueNode::get_geom_prop(node) {
            Some(g) if !g.is_empty() => g,
            _ => return,
        };
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let uniform_name = format!("{}_{}", token::T_GEOMPROP, geom_prop);
        // C++ emitOutput(node.getOutput(), true, false, ...) — includeDeclaration=true
        let type_desc = output.get_type();
        let type_name = crate::gen_shader::source_code_node::glsl_type_name(type_desc.get_name());
        let line = format!(
            "{} {} = {}",
            type_name,
            output.port.get_variable(),
            uniform_name
        );
        stage.append_line(&line);
    }
}
