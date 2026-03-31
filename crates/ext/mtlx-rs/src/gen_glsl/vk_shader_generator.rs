//! VkShaderGenerator -- Vulkan GLSL shader generation (by ref MaterialX VkShaderGenerator).
//! Target "genglsl" uses #version 450, inherits genglsl implementations.
//! Auto-creates VkResourceBindingContext for layout(binding=N) and layout(location=N).

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::{Document, ElementPtr};
use crate::gen_glsl::glsl_family::{GlslFamilyBase, HasImplFactory};
use crate::gen_hw::{HwShaderGenerator, create_shader as hw_create_shader};
use crate::gen_shader::{
    GenContext, ImplementationFactory, ResourceBindingContext, Shader, ShaderGenerator,
    ShaderGraph, ShaderNode, ShaderNodeClassification, TypeSystem, VariableBlock, shader_stage,
};

use super::glsl_emit;
use super::glsl_syntax::GlslSyntax;
use super::vk_resource_binding_context::VkResourceBindingContext;

/// Target identifier for Vulkan GLSL (same as genglsl -- inherits all genglsl implementations)
pub const TARGET: &str = "genglsl";
/// Vulkan GLSL version
pub const VERSION: &str = "450";
/// Implementation lookup uses genglsl (vulkan inherits genglsl)
const IMPL_TARGET: &str = "genglsl";

/// Vulkan GLSL shader generator.
pub struct VkShaderGenerator {
    base: GlslFamilyBase,
    /// Auto-created resource binding context for Vulkan (layout bindings).
    resource_binding_ctx: Rc<RefCell<Box<dyn ResourceBindingContext>>>,
    /// Vertex data interface location for inter-stage binding (matches C++ vertexDataLocation).
    vertex_data_location: i32,
}

impl VkShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut base = GlslFamilyBase::new(GlslSyntax::create(type_system));
        base.register_hw_impls(IMPL_TARGET);
        // Auto-create VkResourceBindingContext(0) matching C++ constructor
        let rbc = VkResourceBindingContext::create(0);
        Self {
            base,
            resource_binding_ctx: Rc::new(RefCell::new(rbc)),
            vertex_data_location: 0,
        }
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

    /// Get vertex data location for inter-stage binding.
    pub fn get_vertex_data_location(&self) -> i32 {
        self.vertex_data_location
    }

    /// Get resource binding context: always returns internal auto-created context.
    /// Matches C++ VkShaderGenerator::getResourceBindingContext which ignores GenContext.
    pub fn get_resource_binding_context_internal(
        &self,
        _context: &GenContext<Self>,
    ) -> Option<Rc<RefCell<Box<dyn ResourceBindingContext>>>> {
        Some(self.resource_binding_ctx.clone())
    }

    pub fn generate(&self, name: &str, element: &ElementPtr, context: &GenContext<Self>) -> Shader {
        // Initialize the auto-created binding context
        self.resource_binding_ctx.borrow_mut().initialize();

        if let Some(doc) = Document::from_element(element) {
            let create_ctx = VkShaderGraphContext::new(context);
            match hw_create_shader(name, element, &doc, &create_ctx) {
                Ok(shader) => {
                    let (graph, stages) = shader.into_parts();
                    return glsl_emit::emit_shader_vk(name, graph, stages, &doc, context, self);
                }
                Err(e) => {
                    log::warn!(
                        "VkShaderGenerator: hw_create_shader failed for '{}': {}",
                        name,
                        e
                    );
                }
            }
        } else {
            log::warn!(
                "VkShaderGenerator: Document::from_element returned None for '{}'",
                name
            );
        }

        // Fallback: trivial black shader when graph creation fails.
        log::warn!(
            "VkShaderGenerator: falling back to trivial shader for '{}'",
            name
        );
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

impl HasImplFactory for VkShaderGenerator {
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

impl ShaderGenerator for VkShaderGenerator {
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

impl HwShaderGenerator for VkShaderGenerator {
    /// Vulkan uses dot notation for interface block member access (matches C++).
    fn get_vertex_data_prefix(&self, vertex_data: &VariableBlock) -> String {
        format!("{}.", vertex_data.get_instance())
    }
}

// --- ShaderGraphContext ---

crate::def_glsl_graph_context!(VkShaderGraphContext, VkShaderGenerator);
crate::impl_glsl_impl_context!(VkShaderGraphContext, VkShaderGenerator);
crate::impl_glsl_graph_ctx!(VkShaderGraphContext, VkShaderGenerator, TARGET, IMPL_TARGET);
