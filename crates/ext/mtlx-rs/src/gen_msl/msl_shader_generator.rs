//! MslShaderGenerator -- Metal Shading Language shader generation (ref: MaterialXGenMsl).
//! Target "genmsl" for Apple Metal, uses genmsl implementations from stdlib.

use std::sync::Arc;

use crate::core::Document;
use crate::core::ElementPtr;
use crate::core::element::category;
use crate::gen_hw::{
    HwBitangentNode, HwFrameNode, HwGeomColorNode, HwGeomPropValueNode,
    HwGeomPropValueNodeAsUniform, HwImageNode, HwLightNode, HwLightShaderNode, HwNormalNode,
    HwPositionNode, HwShaderGenerator, HwSurfaceNode, HwTangentNode, HwTexCoordNode, HwTimeNode,
    HwTransformNormalNode, HwTransformPointNode, HwTransformVectorNode, HwViewDirectionNode,
    create_shader as hw_create_shader,
};
use crate::gen_shader::{
    CompoundNode, GenContext, ImplementationFactory, MaterialNode, Shader, ShaderGenerator,
    ShaderGraph, ShaderGraphCreateContext, ShaderImplContext, TypeSystem, VariableBlock,
    shader_stage,
};

use super::msl_emit;
use super::msl_syntax::MslSyntax;

/// Target identifier for Metal shader generator
pub const TARGET: &str = "genmsl";
/// Metal Shading Language version (Metal 2.3, ref: MslShaderGenerator.cpp)
pub const VERSION: &str = "2.3";
/// Implementation lookup uses genmsl (native MSL implementations)
const IMPL_TARGET: &str = "genmsl";

/// Metal shader generator.
pub struct MslShaderGenerator {
    syntax: MslSyntax,
    impl_factory: ImplementationFactory,
}

impl MslShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut g = Self {
            syntax: MslSyntax::create(type_system),
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
        let target = IMPL_TARGET;

        // <!-- <surfacematerial> -->
        self.impl_factory.register(
            &format!("IM_surfacematerial_{}", target),
            Arc::new(|| MaterialNode::create()),
        );

        // <!-- <position> -->
        self.impl_factory.register(
            &format!("IM_position_vector3_{}", target),
            Arc::new(|| HwPositionNode::create()),
        );
        // <!-- <normal> -->
        self.impl_factory.register(
            &format!("IM_normal_vector3_{}", target),
            Arc::new(|| HwNormalNode::create()),
        );
        // <!-- <tangent> -->
        self.impl_factory.register(
            &format!("IM_tangent_vector3_{}", target),
            Arc::new(|| HwTangentNode::create()),
        );
        // <!-- <bitangent> -->
        self.impl_factory.register(
            &format!("IM_bitangent_vector3_{}", target),
            Arc::new(|| HwBitangentNode::create()),
        );
        // <!-- <viewdirection> -->
        self.impl_factory.register(
            &format!("IM_viewdirection_vector3_{}", target),
            Arc::new(|| HwViewDirectionNode::create()),
        );
        // <!-- <ND_transformpoint> -->
        self.impl_factory.register(
            &format!("IM_transformpoint_vector3_{}", target),
            Arc::new(|| HwTransformPointNode::create()),
        );
        // <!-- <ND_transformvector> -->
        self.impl_factory.register(
            &format!("IM_transformvector_vector3_{}", target),
            Arc::new(|| HwTransformVectorNode::create()),
        );
        // <!-- <ND_transformnormal> -->
        self.impl_factory.register(
            &format!("IM_transformnormal_vector3_{}", target),
            Arc::new(|| HwTransformNormalNode::create()),
        );
        // <!-- <surface> -->
        self.impl_factory.register(
            &format!("IM_surface_{}", target),
            Arc::new(|| HwSurfaceNode::create()),
        );
        // <!-- <light> -->
        self.impl_factory.register(
            &format!("IM_light_{}", target),
            Arc::new(|| HwLightNode::create()),
        );

        // <!-- <point_light> -->
        self.impl_factory.register(
            &format!("IM_point_light_{}", target),
            Arc::new(|| HwLightShaderNode::create()),
        );
        // <!-- <directional_light> -->
        self.impl_factory.register(
            &format!("IM_directional_light_{}", target),
            Arc::new(|| HwLightShaderNode::create()),
        );
        // <!-- <spot_light> -->
        self.impl_factory.register(
            &format!("IM_spot_light_{}", target),
            Arc::new(|| HwLightShaderNode::create()),
        );

        // <!-- <geomcolor> -->
        for ty in ["float", "color3", "color4"] {
            self.impl_factory.register(
                &format!("IM_geomcolor_{}_{}", ty, target),
                Arc::new(|| HwGeomColorNode::create()),
            );
        }
        // <!-- <texcoord> -->
        self.impl_factory.register_multi(
            [
                format!("IM_texcoord_vector2_{}", target),
                format!("IM_texcoord_vector3_{}", target),
            ],
            Arc::new(|| HwTexCoordNode::create()),
        );
        // <!-- <geompropvalue> -->
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
        // <!-- <frame> -->
        self.impl_factory.register(
            &format!("IM_frame_float_{}", target),
            Arc::new(|| HwFrameNode::create()),
        );
        // <!-- <time> -->
        self.impl_factory.register(
            &format!("IM_time_float_{}", target),
            Arc::new(|| HwTimeNode::create()),
        );

        // <!-- <image> --> (6 types, ref: MslShaderGenerator.cpp)
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

    pub fn get_syntax(&self) -> &MslSyntax {
        &self.syntax
    }

    pub fn get_impl_factory(&self) -> &ImplementationFactory {
        &self.impl_factory
    }

    pub fn generate(&self, name: &str, element: &ElementPtr, context: &GenContext<Self>) -> Shader {
        if let Some(rbc) = context.get_resource_binding_context() {
            rbc.borrow_mut().initialize();
        }

        if let Some(doc) = Document::from_element(element) {
            let create_ctx = MslShaderGraphContext::new(context);
            if let Ok(shader) = hw_create_shader(name, element, &doc, &create_ctx) {
                let (graph, stages) = shader.into_parts();
                return msl_emit::emit_shader_msl(name, graph, stages, &doc, context, self);
            }
        }

        let graph = ShaderGraph::new(name);
        let mut shader = Shader::new(name, graph);
        {
            let ps = shader.create_stage(shader_stage::PIXEL);
            ps.append_line("#include <metal_stdlib>");
            ps.append_line("using namespace metal;");
            ps.append_line("fragment float4 fragment_main() { return float4(0.0); }");
        }
        shader
    }
}

impl ShaderGenerator for MslShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.syntax.get_syntax().type_system
    }
    fn target(&self) -> &str {
        TARGET
    }
}

impl HwShaderGenerator for MslShaderGenerator {
    /// Return vertex data prefix: instance + "." (ref: MslShaderGenerator::getVertexDataPrefix).
    /// C++ returns vertexData.getInstance() + "." (typically "vd.").
    fn get_vertex_data_prefix(&self, vertex_data: &VariableBlock) -> String {
        let instance = vertex_data.get_instance();
        if instance.is_empty() {
            "vd.".to_string()
        } else {
            format!("{}.", instance)
        }
    }
}

/// Context for MSL: get_target()=genmsl, get_implementation_target()=genmsl.
pub struct MslShaderGraphContext<'a> {
    pub ctx: &'a GenContext<MslShaderGenerator>,
    graph: Option<&'a ShaderGraph>,
}

impl MslShaderGraphContext<'_> {
    pub fn new(ctx: &GenContext<MslShaderGenerator>) -> MslShaderGraphContext<'_> {
        MslShaderGraphContext { ctx, graph: None }
    }
    pub fn with_graph<'b>(
        ctx: &'b GenContext<MslShaderGenerator>,
        graph: &'b ShaderGraph,
    ) -> MslShaderGraphContext<'b> {
        MslShaderGraphContext {
            ctx,
            graph: Some(graph),
        }
    }
}

impl ShaderImplContext for MslShaderGraphContext<'_> {
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

impl ShaderGraphCreateContext for MslShaderGraphContext<'_> {
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
        IMPL_TARGET
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
    ) -> Option<Box<dyn crate::gen_shader::ShaderNodeImpl>> {
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
                .or_else(|| Some(crate::gen_shader::SourceCodeNode::create()))?;
            impl_.initialize(&impl_elem, self);
            Some(impl_)
        } else {
            None
        }
    }
}
