//! GlslShaderGenerator — GLSL shader generation (по рефу MaterialX GenGlsl GlslShaderGenerator).

use crate::core::{Document, ElementPtr};
use crate::gen_glsl::glsl_family::{GlslFamilyBase, HasImplFactory};
use crate::gen_hw::{HwShaderGenerator, create_shader as hw_create_shader};
use crate::gen_shader::{
    GenContext, ImplementationFactory, Shader, ShaderGenerator, ShaderGraph, ShaderNode,
    ShaderNodeClassification, TypeSystem, VariableBlock, shader_stage,
};

use super::glsl_emit;
use super::glsl_syntax::GlslSyntax;

/// Target identifier for GLSL generator
pub const TARGET: &str = "genglsl";
/// GLSL version
pub const VERSION: &str = "400";

/// GLSL shader generator.
pub struct GlslShaderGenerator {
    base: GlslFamilyBase,
}

impl GlslShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut base = GlslFamilyBase::new(GlslSyntax::create(type_system));
        // GLSL uses its own target as impl_target — no inheritance needed.
        base.register_hw_impls(TARGET);
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
            let create_ctx = GlslShaderGraphContext::new(context);
            if let Ok(shader) = hw_create_shader(name, element, &doc, &create_ctx) {
                let (graph, stages) = shader.into_parts();
                return glsl_emit::emit_shader(name, graph, stages, &doc, context, self);
            }
        }

        // Fallback — minimal valid GLSL shader
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

impl HasImplFactory for GlslShaderGenerator {
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

impl ShaderGenerator for GlslShaderGenerator {
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

impl HwShaderGenerator for GlslShaderGenerator {
    /// C++ uses vertexData.getInstance() + "." for interface block member access.
    fn get_vertex_data_prefix(&self, vertex_data: &VariableBlock) -> String {
        format!("{}.", vertex_data.get_instance())
    }
}

// ─── ShaderGraphContext ──────────────────────────────────────────────────────

crate::def_glsl_graph_context!(GlslShaderGraphContext, GlslShaderGenerator);
crate::impl_glsl_impl_context!(GlslShaderGraphContext, GlslShaderGenerator);
crate::impl_glsl_graph_ctx!(GlslShaderGraphContext, GlslShaderGenerator, TARGET, TARGET);
