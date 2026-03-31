//! OslShaderGenerator — OSL shader generation (по рефу MaterialXGenOsl OslShaderGenerator).
//! Open Shading Language — single-stage shader, surface/volume/shader types.

use crate::core::Document;
use crate::core::ElementPtr;
use crate::core::element::category;
use crate::gen_shader::{
    CompoundNode, GenContext, Shader, ShaderGenerator, ShaderGraphCreateContext, ShaderImplContext,
    ShaderMetadataRegistry, ShaderNodeImpl, ShaderPortMetadata, SourceCodeNode, TypeSystem,
    create_from_element, create_from_nodegraph, shader_stage,
};

use super::osl_syntax::OslSyntax;

/// OSL variable block identifiers (по рефу OslShaderGenerator.cpp OSL namespace)
pub mod osl_block {
    pub const UNIFORMS: &str = "u";
    pub const INPUTS: &str = "i";
    pub const OUTPUTS: &str = "o";
}

/// Target identifier for OSL generator
pub const TARGET: &str = "genosl";

/// OSL shader generator — inherits from base ShaderGenerator (not HwShaderGenerator).
pub struct OslShaderGenerator {
    syntax: OslSyntax,
}

impl OslShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        Self {
            syntax: OslSyntax::create(type_system),
        }
    }

    pub fn create(type_system: Option<TypeSystem>) -> Self {
        let ts = type_system.unwrap_or_else(TypeSystem::new);
        Self::new(ts)
    }

    pub fn get_target(&self) -> &str {
        TARGET
    }

    /// Register shader metadata for OSL (по рефу OslShaderGenerator::registerShaderMetadata).
    /// Call before generate() to set up OSL metadata name mappings (uiname→label, etc.).
    pub fn register_shader_metadata(
        &self,
        _doc: &Document,
        context: &mut GenContext<OslShaderGenerator>,
    ) {
        register_osl_shader_metadata(context);
    }

    pub fn get_syntax(&self) -> &OslSyntax {
        &self.syntax
    }
}

impl ShaderGenerator for OslShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.syntax.get_syntax().type_system
    }
    fn target(&self) -> &str {
        TARGET
    }
}

/// Context for OSL ShaderGraph creation — implements ShaderGraphCreateContext.
pub struct OslShaderGraphContext<'a> {
    pub ctx: &'a GenContext<OslShaderGenerator>,
    graph: Option<&'a crate::gen_shader::ShaderGraph>,
}

impl<'a> OslShaderGraphContext<'a> {
    pub fn new(ctx: &'a GenContext<OslShaderGenerator>) -> Self {
        Self { ctx, graph: None }
    }
    pub fn with_graph(
        ctx: &'a GenContext<OslShaderGenerator>,
        graph: &'a crate::gen_shader::ShaderGraph,
    ) -> Self {
        Self {
            ctx,
            graph: Some(graph),
        }
    }
}

impl ShaderImplContext for OslShaderGraphContext<'_> {
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
        Some(osl_type_for_emit(type_name))
    }
    fn get_default_value(&self, type_name: &str, _as_uniform: bool) -> String {
        osl_type_for_emit(type_name).1.to_string()
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

fn osl_type_for_emit(type_name: &str) -> (&'static str, &'static str) {
    match type_name {
        "float" => ("float", "0.0"),
        "integer" | "boolean" => ("int", "0"),
        "vector2" => ("vector2", "vector2(0.0, 0.0)"),
        "vector3" => ("vector", "vector(0.0)"),
        "vector4" => ("vector4", "vector4(0.0, 0.0, 0.0, 0.0)"),
        "color3" => ("color", "color(0.0)"),
        "color4" => ("color4", "color4(color(0.0), 0.0)"),
        "matrix33" | "matrix44" => ("matrix", "matrix(1.0)"),
        "string" => ("string", "\"\""),
        "filename" => ("textureresource", "textureresource(\"\", \"\")"),
        "surfaceshader" | "material" => (
            "surfaceshader",
            "surfaceshader(null_closure(), null_closure(), 1.0)",
        ),
        "volumeshader" => ("volumeshader", "null_closure()"),
        "displacementshader" => ("vector", "vector(0.0)"),
        _ => ("float", "0.0"),
    }
}

impl ShaderGraphCreateContext for OslShaderGraphContext<'_> {
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
        for impl_elem in impls {
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
                let mut impl_ = SourceCodeNode::create();
                impl_.initialize(&impl_elem, self);
                Some(impl_)
            } else {
                continue;
            };
        }
        None
    }
}

/// Call create_variables for all nodes (по рефу ShaderGenerator::createVariables).
fn create_variables(shader: &mut Shader, doc: &Document, context: &OslShaderGraphContext<'_>) {
    let target = context.get_implementation_target();
    let node_order: Vec<String> = shader.get_graph().node_order.clone();
    let node_def_pairs: Vec<(String, String)> = node_order
        .iter()
        .filter_map(|n| {
            let nd = shader.get_graph().get_node_def(n)?.to_string();
            Some((n.clone(), nd))
        })
        .collect();
    for (node_name, node_def_name) in node_def_pairs {
        if let Some(impl_) = context.get_implementation_for_nodedef(doc, &node_def_name, target) {
            impl_.create_variables(&node_name, context, shader);
        }
    }
}

/// Register shader metadata for OSL (по рефу OslShaderGenerator::registerShaderMetadata).
/// Call before generate() to set up OSL metadata (default entries + uiname→label remapping).
pub fn register_osl_shader_metadata(context: &mut GenContext<OslShaderGenerator>) {
    let mut registry = context
        .shader_metadata_registry
        .take()
        .unwrap_or_else(ShaderMetadataRegistry::new);
    registry.add_default_entries();
    registry.apply_osl_remapping();
    context.shader_metadata_registry = Some(registry);
}

/// Create OSL shader from element (по рефу OslShaderGenerator::createShader).
pub fn create_osl_shader(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &GenContext<OslShaderGenerator>,
) -> Result<Shader, String> {
    let osl_ctx = OslShaderGraphContext::new(context);
    let mut graph = if element.borrow().get_category() == category::NODE_GRAPH {
        create_from_nodegraph(element, doc, &osl_ctx)
    } else {
        create_from_element(name, element, doc, &osl_ctx)
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
                    &osl_ctx,
                )?;
            }
        }
    }

    let mut shader = Shader::new(name, graph);
    shader.create_stage(shader_stage::PIXEL);
    create_variables(&mut shader, doc, &osl_ctx);
    let g = shader.get_graph();
    let graph_name = g.get_name().to_string();
    let mut uniform_adds: Vec<(
        crate::gen_shader::TypeDesc,
        String,
        Option<crate::core::Value>,
        Vec<ShaderPortMetadata>,
    )> = Vec::new();
    for i in 0..g.num_input_sockets() {
        let socket = g.get_input_socket_at(i).ok_or("Input socket")?;
        if !g
            .get_connections_for_output(&graph_name, socket.get_name())
            .is_empty()
            && !socket.port.get_type().is_closure()
            && g.is_editable(socket.get_name())
        {
            let meta: Vec<ShaderPortMetadata> = socket
                .port
                .get_metadata()
                .iter()
                .map(|m| ShaderPortMetadata {
                    name: m.name.clone(),
                    value: m.value.clone(),
                })
                .collect();
            uniform_adds.push((
                socket.port.get_type().clone(),
                socket.port.get_variable().to_string(),
                socket.port.get_value().cloned(),
                meta,
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
    for (td, var, val, meta) in uniform_adds {
        let port = uniforms.add(td, var, val, false);
        for m in meta {
            port.add_metadata(m.name, m.value);
        }
    }

    let outputs = stage.get_output_block_mut(osl_block::OUTPUTS).unwrap();
    for (td, var, val) in output_adds {
        outputs.add(td, var, val, false);
    }

    Ok(shader)
}
