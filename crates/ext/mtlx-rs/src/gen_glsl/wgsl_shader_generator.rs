//! WgslShaderGenerator — WGSL/WebGPU shader generation (по рефу MaterialXGenGlsl WgslShaderGenerator).
//! Inherits from VkShaderGenerator: #version 450, texture+sampler split for WebGPU transpilation.

use crate::core::{Document, ElementPtr};
use crate::gen_glsl::glsl_family::{GlslFamilyBase, HasImplFactory};
use crate::gen_hw::{HwShaderGenerator, create_shader as hw_create_shader};
use crate::gen_shader::{
    GenContext, ImplementationFactory, Shader, ShaderGenerator, ShaderGraph, ShaderImplContext,
    ShaderNode, ShaderNodeClassification, TypeSystem, VariableBlock, shader_stage,
};

use super::glsl_emit;
use super::glsl_syntax::GlslSyntax;
use super::wgsl_syntax::create_wgsl_syntax;

/// Target identifier for WGSL generator
pub const TARGET: &str = "wgsl";
/// Vulkan GLSL version (output transpilable to WGSL)
pub const VERSION: &str = "450";
/// Implementation lookup uses genglsl (wgsl inherits genglsl via vk)
const IMPL_TARGET: &str = "genglsl";

/// WGSL shader generator — Vulkan GLSL with texture+sampler split for WebGPU.
pub struct WgslShaderGenerator {
    base: GlslFamilyBase,
}

impl WgslShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        // WGSL uses a modified syntax with split texture/sampler types
        let mut base = GlslFamilyBase::new(create_wgsl_syntax(type_system));
        base.register_hw_impls(IMPL_TARGET);
        Self { base }
    }

    pub fn create(type_system: Option<TypeSystem>) -> Self {
        Self::new(type_system.unwrap_or_else(TypeSystem::new))
    }

    pub fn get_target(&self) -> &str {
        TARGET
    }
    pub fn get_version(&self) -> &str {
        VERSION
    }
    pub fn get_syntax(&self) -> &GlslSyntax {
        &self.base.syntax
    }
    pub fn get_impl_factory(&self) -> &ImplementationFactory {
        &self.base.impl_factory
    }

    pub fn generate(&self, name: &str, element: &ElementPtr, context: &GenContext<Self>) -> Shader {
        if let Some(rbc) = context.get_resource_binding_context() {
            rbc.borrow_mut().initialize();
        }
        if let Some(doc) = Document::from_element(element) {
            let create_ctx = WgslShaderGraphContext::new(context);
            if let Ok(shader) = hw_create_shader(name, element, &doc, &create_ctx) {
                let (graph, stages) = shader.into_parts();
                return glsl_emit::emit_shader_wgsl(name, graph, stages, &doc, context, self);
            }
        }

        let graph = ShaderGraph::new(name);
        let mut shader = Shader::new(name, graph);
        {
            let ps = shader.create_stage(shader_stage::PIXEL);
            ps.append_line(&format!("#version {}", VERSION));
            ps.append_line("out vec4 fragColor;");
            ps.append_line("void main() { fragColor = vec4(0.0); }");
        }
        shader
    }
}

impl HasImplFactory for WgslShaderGenerator {
    fn get_impl_factory(&self) -> &ImplementationFactory {
        &self.base.impl_factory
    }
    fn get_syntax_base(&self) -> &GlslSyntax {
        &self.base.syntax
    }
    fn get_type_system_ref(&self) -> &TypeSystem {
        &self.base.syntax.get_syntax().type_system
    }
}

impl ShaderGenerator for WgslShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.base.syntax.get_syntax().type_system
    }
    fn target(&self) -> &str {
        TARGET
    }
    /// C++ HwShaderGenerator::nodeNeedsClosureData — true for BSDF/EDF/VDF nodes.
    fn node_needs_closure_data(&self, node: &ShaderNode) -> bool {
        node.has_classification(ShaderNodeClassification::BSDF)
            || node.has_classification(ShaderNodeClassification::EDF)
            || node.has_classification(ShaderNodeClassification::VDF)
    }
}

impl HwShaderGenerator for WgslShaderGenerator {
    /// C++ WgslShaderGenerator inherits VkShaderGenerator: vertexData.getInstance() + "."
    fn get_vertex_data_prefix(&self, vertex_data: &VariableBlock) -> String {
        format!("{}.", vertex_data.get_instance())
    }

    /// C++ WgslShaderGenerator::LIGHTDATA_TYPEVAR_STRING = "light_type".
    /// `type` is a WGSL reserved word — must use `light_type` instead.
    fn get_light_data_type_var_string(&self) -> &str {
        "light_type"
    }
}

// ─── ShaderGraphContext ──────────────────────────────────────────────────────

crate::def_glsl_graph_context!(WgslShaderGraphContext, WgslShaderGenerator);

// WGSL overrides format_filename_arg for texture+sampler split — must write ShaderImplContext manually.
// All methods from impl_glsl_impl_context! macro must be replicated here.
impl ShaderImplContext for WgslShaderGraphContext<'_> {
    fn resolve_source_file(
        &self,
        filename: &str,
        local_path: Option<&crate::format::FilePath>,
    ) -> Option<crate::format::FilePath> {
        self.ctx.resolve_source_file(filename, local_path)
    }
    fn get_graph(&self) -> Option<&ShaderGraph> {
        self.graph
    }
    fn get_type_system(&self) -> &TypeSystem {
        self.ctx.get_type_system()
    }
    fn get_default_value(&self, type_name: &str, as_uniform: bool) -> String {
        let td = self.ctx.get_type_desc(type_name);
        self.ctx
            .get_shader_generator()
            .get_syntax()
            .get_syntax()
            .get_default_value(&td, as_uniform)
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
    /// C++ closureData argument injection for BSDF/EDF/VDF nodes.
    fn get_closure_data_argument(&self, node: &ShaderNode) -> Option<String> {
        if <WgslShaderGenerator as ShaderGenerator>::node_needs_closure_data(
            self.ctx.get_shader_generator(),
            node,
        ) {
            Some(crate::gen_hw::hw_lighting::CLOSURE_DATA_ARG.to_string())
        } else {
            None
        }
    }
    /// WGSL splits combined sampler2D into texture+sampler pair for WebGPU transpilation.
    fn format_filename_arg(&self, var: &str) -> String {
        format!("{}_texture, {}_sampler", var, var)
    }
    fn get_gen_options(&self) -> &crate::gen_shader::GenOptions {
        self.ctx.get_options()
    }
    fn as_graph_create_context(&self) -> Option<&dyn crate::gen_shader::ShaderGraphCreateContext> {
        Some(self)
    }
    /// Re-emit a node's function call: resolves impl via doc+ShaderGraphCreateContext,
    /// then calls emit_function_call. C++ shadergen.emitFunctionCall(*node, ctx, stage).
    fn emit_node_function_call(&self, node_name: &str, stage: &mut crate::gen_shader::ShaderStage) {
        let graph = match self.graph {
            Some(g) => g,
            None => return,
        };
        let doc = match self.doc {
            Some(d) => d,
            None => return,
        };
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => return,
        };
        let node_def_name = match graph.get_node_def(node_name) {
            Some(nd) => nd,
            None => return,
        };
        // get_implementation_target/get_implementation_for_nodedef live on ShaderGraphCreateContext.
        let gc = match self.as_graph_create_context() {
            Some(g) => g,
            None => return,
        };
        let target = gc.get_implementation_target();
        let impl_opt = gc.get_implementation_for_nodedef(doc, node_def_name, target);
        let impl_ = match impl_opt {
            Some(i) => i,
            None => return,
        };
        impl_.emit_function_call(node, self, stage);
    }
}

crate::impl_glsl_graph_ctx!(
    WgslShaderGraphContext,
    WgslShaderGenerator,
    TARGET,
    IMPL_TARGET
);
