//! SlangShaderGenerator — Slang shader generation (по рефу MaterialXGenSlang).
//! Slang extends HwShaderGenerator; uses same createShader, different emit.

use std::sync::Arc;

use crate::core::Document;
use crate::core::ElementPtr;
use crate::core::element::category;
use crate::gen_hw::{
    HwBitangentNode, HwFrameNode, HwGeomColorNode, HwGeomPropValueNode,
    HwGeomPropValueNodeAsUniform, HwImageNode, HwLightNode, HwLightShaderNode, HwNormalNode,
    HwPositionNode, HwSurfaceNode, HwTangentNode, HwTexCoordNode, HwTimeNode,
    HwTransformNormalNode, HwTransformPointNode, HwTransformVectorNode, HwViewDirectionNode,
    create_shader as hw_create_shader,
};
use crate::gen_shader::{
    CompoundNode, GenContext, ImplementationFactory, MaterialNode, Shader, ShaderGenerator,
    ShaderGraphCreateContext, ShaderImplContext, ShaderMetadataRegistry, ShaderNodeImpl,
    SourceCodeNode, TypeSystem,
};

use super::slang_syntax::SlangSyntax;

/// Target identifier for Slang generator
pub const TARGET: &str = "genslang";
/// Slang version
pub const VERSION: &str = "2025.3";

/// Slang shader generator — uses Hw createShader, Slang syntax.
pub struct SlangShaderGenerator {
    syntax: SlangSyntax,
    impl_factory: ImplementationFactory,
}

impl SlangShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut g = Self {
            syntax: SlangSyntax::create(type_system),
            impl_factory: ImplementationFactory::new(),
        };
        g.register_implementations();
        g
    }

    pub fn create(type_system: Option<TypeSystem>) -> Self {
        let ts = type_system.unwrap_or_else(TypeSystem::new);
        Self::new(ts)
    }

    fn register_implementations(&mut self) {
        let target = TARGET;
        self.impl_factory.register(
            &format!("IM_surfacematerial_{}", target),
            Arc::new(|| MaterialNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_position_vector3_{}", target),
            Arc::new(|| HwPositionNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_normal_vector3_{}", target),
            Arc::new(|| HwNormalNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_tangent_vector3_{}", target),
            Arc::new(|| HwTangentNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_bitangent_vector3_{}", target),
            Arc::new(|| HwBitangentNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_viewdirection_vector3_{}", target),
            Arc::new(|| HwViewDirectionNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_transformpoint_vector3_{}", target),
            Arc::new(|| HwTransformPointNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_transformvector_vector3_{}", target),
            Arc::new(|| HwTransformVectorNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_transformnormal_vector3_{}", target),
            Arc::new(|| HwTransformNormalNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_surface_{}", target),
            Arc::new(|| HwSurfaceNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_light_{}", target),
            Arc::new(|| HwLightNode::create()),
        );
        for ty in ["float", "color3", "color4"] {
            self.impl_factory.register(
                &format!("IM_geomcolor_{}_{}", ty, target),
                Arc::new(|| HwGeomColorNode::create()),
            );
        }
        self.impl_factory.register_multi(
            [
                format!("IM_texcoord_vector2_{}", target),
                format!("IM_texcoord_vector3_{}", target),
            ],
            Arc::new(|| HwTexCoordNode::create()),
        );
        for ty in [
            "integer", "float", "color3", "color4", "vector2", "vector3", "vector4",
        ] {
            self.impl_factory.register(
                &format!("IM_geompropvalue_{}_{}", ty, target),
                Arc::new(|| HwGeomPropValueNode::create()),
            );
        }
        self.impl_factory.register(
            &format!("IM_geompropvalue_boolean_{}", target),
            Arc::new(|| HwGeomPropValueNodeAsUniform::create()),
        );
        self.impl_factory.register(
            &format!("IM_geompropvalue_string_{}", target),
            Arc::new(|| HwGeomPropValueNodeAsUniform::create()),
        );
        self.impl_factory.register(
            &format!("IM_geompropvalue_filename_{}", target),
            Arc::new(|| HwGeomPropValueNodeAsUniform::create()),
        );
        self.impl_factory.register(
            &format!("IM_frame_float_{}", target),
            Arc::new(|| HwFrameNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_time_float_{}", target),
            Arc::new(|| HwTimeNode::create()),
        );
        // Light shader nodes (ref: IM_point_light, IM_directional_light, IM_spot_light)
        for light in ["point_light", "directional_light", "spot_light"] {
            self.impl_factory.register(
                &format!("IM_{}_{}", light, target),
                Arc::new(|| HwLightShaderNode::create()),
            );
        }
        // Image nodes (ref: HwImageNode)
        for ty in ["float", "color3", "color4", "vector2", "vector3", "vector4"] {
            self.impl_factory.register(
                &format!("IM_image_{}_{}", ty, target),
                Arc::new(|| HwImageNode::create()),
            );
        }
    }

    pub fn get_target(&self) -> &str {
        TARGET
    }

    pub fn get_version(&self) -> &str {
        VERSION
    }

    pub fn get_syntax(&self) -> &SlangSyntax {
        &self.syntax
    }

    pub fn get_impl_factory(&self) -> &ImplementationFactory {
        &self.impl_factory
    }

    /// Generate Slang shader from element (creates shader + emits Slang source).
    pub fn generate(
        &self,
        name: &str,
        element: &ElementPtr,
        context: &mut GenContext<SlangShaderGenerator>,
    ) -> Result<Shader, String> {
        let doc =
            Document::from_element(element).ok_or_else(|| "Element has no document".to_string())?;
        generate_slang_shader(name, element, &doc, context)
    }
}

impl ShaderGenerator for SlangShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.syntax.get_syntax().type_system
    }
    fn target(&self) -> &str {
        TARGET
    }
}

/// Context for Slang ShaderGraph creation and emit.
pub struct SlangShaderGraphContext<'a> {
    pub ctx: &'a GenContext<SlangShaderGenerator>,
    graph: Option<&'a crate::gen_shader::ShaderGraph>,
}

impl<'a> SlangShaderGraphContext<'a> {
    pub fn new(ctx: &'a GenContext<SlangShaderGenerator>) -> Self {
        Self { ctx, graph: None }
    }
    pub fn with_graph<'b>(
        ctx: &'b GenContext<SlangShaderGenerator>,
        graph: &'b crate::gen_shader::ShaderGraph,
    ) -> SlangShaderGraphContext<'b> {
        SlangShaderGraphContext {
            ctx,
            graph: Some(graph),
        }
    }
}

impl ShaderImplContext for SlangShaderGraphContext<'_> {
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
    fn get_closure_data_argument(&self, node: &crate::gen_shader::ShaderNode) -> Option<String> {
        if self
            .ctx
            .get_shader_generator()
            .node_needs_closure_data(node)
        {
            Some(crate::gen_hw::hw_lighting::CLOSURE_DATA_ARG.to_string())
        } else {
            None
        }
    }
    fn get_gen_options(&self) -> &crate::gen_shader::GenOptions {
        self.ctx.get_options()
    }
    fn as_graph_create_context(&self) -> Option<&dyn ShaderGraphCreateContext> {
        Some(self)
    }
}

impl ShaderGraphCreateContext for SlangShaderGraphContext<'_> {
    fn get_shader_metadata_registry(&self) -> Option<&ShaderMetadataRegistry> {
        None
    }
    fn get_syntax(&self) -> &crate::gen_shader::Syntax {
        self.ctx.get_shader_generator().get_syntax().get_syntax()
    }
    fn get_options(&self) -> &crate::gen_shader::GenOptions {
        self.ctx.get_options()
    }
    fn get_target(&self) -> &str {
        TARGET
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
        let impls = doc.get_matching_implementations(node_def_name);
        let impl_elem = impls.into_iter().next()?;
        let impl_name = impl_elem.borrow().get_name().to_string();
        let cat = impl_elem.borrow().get_category().to_string();
        let impl_target = impl_elem
            .borrow()
            .get_attribute("target")
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !impl_target.is_empty() && impl_target != target {
            return None;
        }
        if cat == category::NODE_GRAPH {
            let mut compound = CompoundNode::create();
            compound.initialize(&impl_elem, self);
            Some(compound)
        } else if cat == category::IMPLEMENTATION {
            let mut impl_ = self
                .ctx
                .get_shader_generator()
                .get_impl_factory()
                .create(&impl_name)
                .or_else(|| Some(SourceCodeNode::create()))?;
            impl_.initialize(&impl_elem, self);
            Some(impl_)
        } else {
            None
        }
    }
}

/// Create Slang shader — delegates to Hw createShader (по рефу Slang extends Hw).
pub fn create_slang_shader(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &GenContext<SlangShaderGenerator>,
) -> Result<Shader, String> {
    let slang_ctx = SlangShaderGraphContext::new(context);
    hw_create_shader(name, element, doc, &slang_ctx)
}

/// Generate Slang shader — creates shader and emits Slang source.
pub fn generate_slang_shader(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &mut GenContext<SlangShaderGenerator>,
) -> Result<Shader, String> {
    let shader = create_slang_shader(name, element, doc, context)?;
    let (graph, stages) = shader.into_parts();
    let generator = context.get_shader_generator();
    let emitted = super::slang_emit::emit_shader(name, graph, stages, doc, context, generator);
    Ok(emitted)
}
