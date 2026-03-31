//! OslNetworkShaderGenerator — OSL network format (param, connect, shader).
//! По рефу MaterialXGenOsl OslNetworkShaderGenerator.

use crate::core::Document;
use crate::core::ElementPtr;
use crate::core::element::category;
use crate::gen_shader::{
    CompoundNode, GenContext, Shader, ShaderGenerator, ShaderGraphCreateContext, ShaderImplContext,
    ShaderMetadataRegistry, ShaderNodeImpl, TypeSystem, create_from_element, create_from_nodegraph,
    shader_stage,
};

use super::nodes::OsoNode;
use super::osl_block;
use super::osl_network_syntax::OslNetworkSyntax;

/// Target for OSL network generator
pub const TARGET: &str = "genoslnetwork";

/// OSL network shader generator — emits param/connect/shader lines instead of full OSL source.
pub struct OslNetworkShaderGenerator {
    syntax: OslNetworkSyntax,
}

impl OslNetworkShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        Self {
            syntax: OslNetworkSyntax::create(type_system),
        }
    }

    pub fn create(type_system: Option<TypeSystem>) -> Self {
        let ts = type_system.unwrap_or_else(TypeSystem::new);
        Self::new(ts)
    }

    pub fn get_target(&self) -> &str {
        TARGET
    }

    pub fn get_syntax(&self) -> &OslNetworkSyntax {
        &self.syntax
    }
}

impl ShaderGenerator for OslNetworkShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.syntax.get_syntax().type_system
    }
    fn target(&self) -> &str {
        TARGET
    }
}

/// Context for OSL network ShaderGraph creation — returns OsoNode for all nodes.
pub struct OslNetworkShaderGraphContext<'a> {
    pub ctx: &'a GenContext<OslNetworkShaderGenerator>,
    graph: Option<&'a crate::gen_shader::ShaderGraph>,
}

impl<'a> OslNetworkShaderGraphContext<'a> {
    pub fn new(ctx: &'a GenContext<OslNetworkShaderGenerator>) -> Self {
        Self { ctx, graph: None }
    }

    pub fn with_graph(
        ctx: &'a GenContext<OslNetworkShaderGenerator>,
        graph: &'a crate::gen_shader::ShaderGraph,
    ) -> Self {
        Self {
            ctx,
            graph: Some(graph),
        }
    }
}

impl ShaderImplContext for OslNetworkShaderGraphContext<'_> {
    fn resolve_source_file(
        &self,
        filename: &str,
        local_path: Option<&crate::format::FilePath>,
    ) -> Option<crate::format::FilePath> {
        self.ctx.resolve_source_file(filename, local_path)
    }
    fn get_type_system(&self) -> &TypeSystem {
        self.ctx.get_type_system()
    }
    fn get_graph(&self) -> Option<&crate::gen_shader::ShaderGraph> {
        self.graph
    }
    fn get_type_name_for_emit(&self, type_name: &str) -> Option<(&'static str, &'static str)> {
        Some((
            match type_name {
                "float" => "float",
                "integer" | "boolean" => "int",
                "vector2" => "vector2",
                "vector3" => "vector",
                "vector4" => "vector4",
                "color3" => "color",
                "color4" => "color",
                "matrix33" | "matrix44" => "matrix",
                "string" => "string",
                "filename" => "textureresource",
                "surfaceshader" | "material" => "surfaceshader",
                "volumeshader" => "volumeshader",
                "displacementshader" => "vector",
                _ => "float",
            },
            match type_name {
                "float" => "0.0",
                "integer" | "boolean" => "0",
                "vector2" => "vector2(0.0, 0.0)",
                "vector3" => "vector(0.0)",
                "vector4" => "vector4(0.0, 0.0, 0.0, 0.0)",
                "color3" => "color(0.0)",
                "color4" => "color4(color(0.0), 0.0)",
                "matrix33" | "matrix44" => "matrix(1.0)",
                "string" => "\"\"",
                "filename" => "textureresource(\"\", \"\")",
                "surfaceshader" | "material" => {
                    "surfaceshader(null_closure(), null_closure(), 1.0)"
                }
                "volumeshader" => "null_closure()",
                "displacementshader" => "vector(0.0)",
                _ => "0.0",
            },
        ))
    }
    fn get_default_value(&self, type_name: &str, _as_uniform: bool) -> String {
        self.get_type_name_for_emit(type_name)
            .map(|(_, default_value)| default_value.to_string())
            .unwrap_or_else(|| "0.0".to_string())
    }
    fn make_valid_name(&self, name: &mut String) {
        self.ctx
            .get_shader_generator()
            .get_syntax()
            .get_syntax()
            .make_valid_name(name);
    }
    fn get_constant_qualifier(&self) -> &str {
        self.ctx
            .get_shader_generator()
            .get_syntax()
            .get_syntax()
            .get_constant_qualifier()
    }
    fn get_gen_options(&self) -> &crate::gen_shader::GenOptions {
        self.ctx.get_options()
    }
    fn as_graph_create_context(&self) -> Option<&dyn ShaderGraphCreateContext> {
        Some(self)
    }
}

impl ShaderGraphCreateContext for OslNetworkShaderGraphContext<'_> {
    fn get_shader_metadata_registry(&self) -> Option<&ShaderMetadataRegistry> {
        self.ctx.shader_metadata_registry.as_ref()
    }
    fn get_syntax(&self) -> &crate::gen_shader::Syntax {
        self.ctx.get_shader_generator().get_syntax().get_syntax()
    }
    fn get_options(&self) -> &crate::gen_shader::GenOptions {
        self.ctx.get_options()
    }
    fn get_target(&self) -> &str {
        self.ctx.get_shader_generator().get_target()
    }
    fn get_implementation_target(&self) -> &str {
        TARGET
    }
    fn get_color_management_system(&self) -> Option<&dyn crate::gen_shader::ColorManagementSystem> {
        self.ctx.get_color_management_system()
    }
    fn get_unit_system(&self) -> Option<&dyn crate::gen_shader::UnitSystem> {
        self.ctx.get_unit_system()
    }
    fn get_implementation_for_nodedef(
        &self,
        doc: &Document,
        node_def_name: &str,
        target: &str,
    ) -> Option<Box<dyn ShaderNodeImpl>> {
        if target != TARGET {
            return None;
        }
        let impls = doc.get_matching_implementations(node_def_name);
        for impl_elem in &impls {
            let impl_target = impl_elem
                .borrow()
                .get_attribute("target")
                .map(|s| s.to_string())
                .unwrap_or_default();
            if !impl_target.is_empty() && impl_target != TARGET {
                continue;
            }
            let cat = impl_elem.borrow().get_category().to_string();
            return if cat == category::NODE_GRAPH {
                let mut compound = CompoundNode::create();
                compound.initialize(&impl_elem, self);
                Some(compound)
            } else if cat == category::IMPLEMENTATION {
                let mut node = OsoNode::new();
                if impl_target == TARGET {
                    node.initialize(&impl_elem, self);
                } else {
                    node.initialize_from_genosl_fallback(&impl_elem, self);
                }
                Some(Box::new(node))
            } else {
                continue;
            };
        }
        // Fallback: try genosl implementations
        for impl_elem in &impls {
            let impl_target = impl_elem
                .borrow()
                .get_attribute("target")
                .map(|s| s.to_string());
            if impl_target.as_deref() == Some(super::osl_shader_generator::TARGET) {
                let cat = impl_elem.borrow().get_category().to_string();
                if cat == category::IMPLEMENTATION {
                    let mut node = OsoNode::new();
                    node.initialize_from_genosl_fallback(&impl_elem, self.ctx);
                    return Some(Box::new(node));
                }
            }
        }
        None
    }
}

/// Create OSL network shader (same structure as create_osl_shader, uses network context).
pub fn create_osl_network_shader(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &GenContext<OslNetworkShaderGenerator>,
) -> Result<Shader, String> {
    let net_ctx = OslNetworkShaderGraphContext::new(context);
    let mut graph = if element.borrow().get_category() == category::NODE_GRAPH {
        create_from_nodegraph(element, doc, &net_ctx)
    } else {
        create_from_element(name, element, doc, &net_ctx)
    }?;

    let opts = context.get_options();
    if opts.osl_implicit_surface_shader_conversion && graph.num_output_sockets() == 1 {
        if let Some(socket) = graph.get_output_socket_at(0) {
            if socket.port.get_type().get_name() == "surfaceshader" {
                graph.inline_node_before_output(
                    0,
                    "_surfacematerial_",
                    "ND_surfacematerial",
                    "surfaceshader",
                    "out",
                    doc,
                    &net_ctx,
                )?;
            }
        }
    }

    let mut shader = Shader::new(name, graph);
    shader.create_stage(shader_stage::PIXEL);
    // create_variables: OsoNode/CompoundNode use default no-op; uniforms set below from graph sockets
    let g = shader.get_graph();
    let graph_name = g.get_name().to_string();
    let mut uniform_adds: Vec<(
        crate::gen_shader::TypeDesc,
        String,
        Option<crate::core::Value>,
    )> = Vec::new();
    for i in 0..g.num_input_sockets() {
        let socket = g.get_input_socket_at(i).ok_or("Input socket")?;
        if !g
            .get_connections_for_output(&graph_name, socket.get_name())
            .is_empty()
            && !socket.port.get_type().is_closure()
            && g.is_editable(socket.get_name())
        {
            uniform_adds.push((
                socket.port.get_type().clone(),
                socket.port.get_variable().to_string(),
                socket.port.get_value().cloned(),
            ));
        }
    }
    let mut output_adds: Vec<(
        crate::gen_shader::TypeDesc,
        String,
        Option<crate::core::Value>,
    )> = Vec::new();
    for i in 0..g.num_output_sockets() {
        let socket = g.get_output_socket_at(i).ok_or("Output socket")?;
        output_adds.push((
            socket.port.get_type().clone(),
            socket.port.get_variable().to_string(),
            socket.port.get_value().cloned(),
        ));
    }

    let stage = shader
        .get_stage_by_name_mut(shader_stage::PIXEL)
        .ok_or("Pixel stage missing")?;
    stage.create_uniform_block(osl_block::UNIFORMS, osl_block::UNIFORMS);
    stage.create_input_block(osl_block::INPUTS, osl_block::INPUTS);
    stage.create_output_block(osl_block::OUTPUTS, osl_block::OUTPUTS);

    let uniforms = stage.get_uniform_block_mut(osl_block::UNIFORMS).unwrap();
    for (td, var, val) in uniform_adds {
        uniforms.add(td, var, val, false);
    }
    let outputs = stage.get_output_block_mut(osl_block::OUTPUTS).unwrap();
    for (td, var, val) in output_adds {
        outputs.add(td, var, val, false);
    }

    Ok(shader)
}
