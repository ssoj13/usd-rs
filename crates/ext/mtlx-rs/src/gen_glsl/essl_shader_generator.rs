//! EsslShaderGenerator — ESSL (WebGL) shader generation (по рефу MaterialX EsslShaderGenerator).
//! Target "essl" inherits from "genglsl", uses #version 300 es (WebGL 2).

use crate::core::{Document, ElementPtr};
use crate::gen_glsl::glsl_family::{GlslFamilyBase, HasImplFactory};
use crate::gen_hw::{HwShaderGenerator, create_shader as hw_create_shader};
use crate::gen_shader::{
    GenContext, ImplementationFactory, Shader, ShaderGenerator, ShaderGraph, ShaderNode,
    ShaderNodeClassification, TypeSystem, VariableBlock, shader_stage,
};

use super::glsl_emit;
use super::glsl_syntax::GlslSyntax;

/// Target identifier for ESSL generator
pub const TARGET: &str = "essl";
/// ESSL version (WebGL 2)
pub const VERSION: &str = "300 es";
/// Implementation lookup uses genglsl (essl inherits genglsl)
const IMPL_TARGET: &str = "genglsl";

/// ESSL (WebGL) shader generator.
pub struct EsslShaderGenerator {
    base: GlslFamilyBase,
}

impl EsslShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut base = GlslFamilyBase::new(GlslSyntax::create(type_system));
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
            let create_ctx = EsslShaderGraphContext::new(context);
            if let Ok(shader) = hw_create_shader(name, element, &doc, &create_ctx) {
                let (graph, stages) = shader.into_parts();
                return glsl_emit::emit_shader_essl(name, graph, stages, &doc, context, self);
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

impl HasImplFactory for EsslShaderGenerator {
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

impl ShaderGenerator for EsslShaderGenerator {
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

impl HwShaderGenerator for EsslShaderGenerator {
    /// ESSL doesn't use interface blocks — no prefix (C++ returns EMPTY_STRING).
    fn get_vertex_data_prefix(&self, _vertex_data: &VariableBlock) -> String {
        String::new()
    }
}

// ─── ShaderGraphContext ──────────────────────────────────────────────────────

crate::def_glsl_graph_context!(EsslShaderGraphContext, EsslShaderGenerator);
crate::impl_glsl_impl_context!(EsslShaderGraphContext, EsslShaderGenerator);
crate::impl_glsl_graph_ctx!(
    EsslShaderGraphContext,
    EsslShaderGenerator,
    TARGET,
    IMPL_TARGET
);
