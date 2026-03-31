//! MdlShaderGenerator -- MDL shader generation (ref: MaterialXGenMdl MdlShaderGenerator).

use crate::core::Document;
use crate::core::ElementPtr;
use crate::core::element::category;
use crate::gen_shader::{
    GenContext, Semantic, Shader, ShaderGenerator, ShaderGraph, ShaderGraphCreateContext,
    ShaderImplContext, ShaderMetadataRegistry, ShaderNodeClassification, ShaderNodeImpl,
    TypeSystem, create_from_element, create_from_nodegraph, shader_stage,
};

use super::closure_compound_node_mdl::ClosureCompoundNodeMdl;
use super::closure_layer_node_mdl::ClosureLayerNodeMdl;
use super::compound_node_mdl::CompoundNodeMdl;
use super::custom_node_mdl::CustomCodeNodeMdl;
use super::height_to_normal_node_mdl::HeightToNormalNodeMdl;
use super::image_node_mdl::ImageNodeMdl;
use super::layerable_node_mdl::LayerableNodeMdl;
use super::material_node_mdl::MaterialNodeMdl;
use super::mdl_syntax::MdlSyntax;
use super::source_code_node_mdl::SourceCodeNodeMdl;
use super::surface_node_mdl::SurfaceNodeMdl;

/// MDL variable block identifiers (ref: MDL namespace)
pub mod mdl_block {
    pub const INPUTS: &str = "i";
    pub const OUTPUTS: &str = "o";
}

/// Target identifier for MDL generator
pub const TARGET: &str = "genmdl";

/// MDL versions supported by the code generator (ref: GenMdlOptions::MdlVersion).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdlVersion {
    Mdl1_6,
    Mdl1_7,
    Mdl1_8,
    Mdl1_9,
    Mdl1_10,
}

impl MdlVersion {
    pub const LATEST: MdlVersion = MdlVersion::Mdl1_10;

    /// Get the version number string (e.g. "1.10")
    pub fn version_string(&self) -> &'static str {
        match self {
            MdlVersion::Mdl1_6 => "1.6",
            MdlVersion::Mdl1_7 => "1.7",
            MdlVersion::Mdl1_8 => "1.8",
            MdlVersion::Mdl1_9 => "1.9",
            MdlVersion::Mdl1_10 => "1.10",
        }
    }

    /// Get the filename suffix string (e.g. "1_10")
    pub fn suffix_string(&self) -> &'static str {
        match self {
            MdlVersion::Mdl1_6 => "1_6",
            MdlVersion::Mdl1_7 => "1_7",
            MdlVersion::Mdl1_8 => "1_8",
            MdlVersion::Mdl1_9 => "1_9",
            MdlVersion::Mdl1_10 => "1_10",
        }
    }

    /// Returns true if uniform IOR is required (MDL < 1.9)
    pub fn requires_uniform_ior(&self) -> bool {
        matches!(
            self,
            MdlVersion::Mdl1_6 | MdlVersion::Mdl1_7 | MdlVersion::Mdl1_8
        )
    }
}

/// Generator context data for MDL options (ref: GenMdlOptions).
#[derive(Debug, Clone)]
pub struct GenMdlOptions {
    /// The MDL version for the generated module (default: LATEST)
    pub target_version: MdlVersion,
}

impl Default for GenMdlOptions {
    fn default() -> Self {
        Self {
            target_version: MdlVersion::LATEST,
        }
    }
}

impl GenMdlOptions {
    /// Unique identifier for the MDL options on the GenContext (ref: GEN_CONTEXT_USER_DATA_KEY)
    pub const USER_DATA_KEY: &'static str = "genmdloptions";
}

/// GEOMPROP default values for MDL (ref: MdlShaderGenerator::GEOMPROP_DEFINITIONS)
pub fn geomprop_default(geomprop: &str) -> Option<&'static str> {
    match geomprop {
        "Pobject" => Some(
            "state::transform_point(state::coordinate_internal, state::coordinate_object, state::position())",
        ),
        "Pworld" => Some(
            "state::transform_point(state::coordinate_internal, state::coordinate_world, state::position())",
        ),
        "Nobject" => Some(
            "state::transform_normal(state::coordinate_internal, state::coordinate_object, state::normal())",
        ),
        "Nworld" => Some(
            "state::transform_normal(state::coordinate_internal, state::coordinate_world, state::normal())",
        ),
        "Tobject" => Some(
            "state::transform_vector(state::coordinate_internal, state::coordinate_object, state::texture_tangent_u(0))",
        ),
        "Tworld" => Some(
            "state::transform_vector(state::coordinate_internal, state::coordinate_world, state::texture_tangent_u(0))",
        ),
        "Bobject" => Some(
            "state::transform_vector(state::coordinate_internal, state::coordinate_object, state::texture_tangent_v(0))",
        ),
        "Bworld" => Some(
            "state::transform_vector(state::coordinate_internal, state::coordinate_world, state::texture_tangent_v(0))",
        ),
        "UV0" => Some("float2(state::texture_coordinate(0).x, state::texture_coordinate(0).y)"),
        "Vworld" => Some("state::direction()"),
        _ => None,
    }
}

/// MDL shader generator -- Material Definition Language (NVIDIA).
pub struct MdlShaderGenerator {
    syntax: MdlSyntax,
    mdl_options: GenMdlOptions,
}

impl MdlShaderGenerator {
    pub fn new(type_system: TypeSystem) -> Self {
        Self {
            syntax: MdlSyntax::create(type_system),
            mdl_options: GenMdlOptions::default(),
        }
    }

    pub fn create(type_system: Option<TypeSystem>) -> Self {
        let ts = type_system.unwrap_or_else(TypeSystem::new);
        Self::new(ts)
    }

    pub fn get_target(&self) -> &str {
        TARGET
    }

    pub fn get_syntax(&self) -> &MdlSyntax {
        &self.syntax
    }

    pub fn get_mdl_options(&self) -> &GenMdlOptions {
        &self.mdl_options
    }

    pub fn set_mdl_options(&mut self, options: GenMdlOptions) {
        self.mdl_options = options;
    }

    /// Get the selected MDL target version (ref: getMdlVersion).
    pub fn get_mdl_version(&self) -> MdlVersion {
        self.mdl_options.target_version
    }

    /// Get the MDL version filename suffix (ref: getMdlVersionFilenameSuffix).
    pub fn get_mdl_version_filename_suffix(&self) -> &'static str {
        self.mdl_options.target_version.suffix_string()
    }

    /// Get the MDL version number string (ref: emitMdlVersionNumber).
    pub fn get_mdl_version_number(&self) -> &'static str {
        self.mdl_options.target_version.version_string()
    }
}

impl ShaderGenerator for MdlShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.syntax.get_syntax().type_system
    }
    fn target(&self) -> &str {
        TARGET
    }
}

/// Context for MDL ShaderGraph creation.
pub struct MdlShaderGraphContext<'a> {
    pub ctx: &'a GenContext<MdlShaderGenerator>,
    graph: Option<&'a ShaderGraph>,
}

impl<'a> MdlShaderGraphContext<'a> {
    pub fn new(ctx: &'a GenContext<MdlShaderGenerator>) -> Self {
        Self { ctx, graph: None }
    }

    pub fn with_graph(ctx: &'a GenContext<MdlShaderGenerator>, graph: &'a ShaderGraph) -> Self {
        Self {
            ctx,
            graph: Some(graph),
        }
    }
}

impl ShaderImplContext for MdlShaderGraphContext<'_> {
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
    fn get_graph(&self) -> Option<&ShaderGraph> {
        self.graph
    }
    fn get_type_name_for_emit(&self, type_name: &str) -> Option<(&'static str, &'static str)> {
        Some(mdl_type_for_emit(type_name))
    }
    fn get_reserved_words(&self) -> Option<&std::collections::HashSet<String>> {
        Some(
            self.ctx
                .get_shader_generator()
                .get_syntax()
                .get_syntax()
                .get_reserved_words(),
        )
    }
    fn make_valid_name(&self, name: &mut String) {
        self.ctx
            .get_shader_generator()
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
    fn get_substitution_tokens(&self) -> Vec<(String, String)> {
        let suffix = self
            .ctx
            .get_shader_generator()
            .get_mdl_version_filename_suffix();
        vec![("{{MDL_VERSION_SUFFIX}}".to_string(), suffix.to_string())]
    }
    fn get_mdl_version_suffix(&self) -> &str {
        self.ctx
            .get_shader_generator()
            .get_mdl_version_filename_suffix()
    }
    fn get_default_value(&self, type_name: &str, as_uniform: bool) -> String {
        let td = self.ctx.get_type_desc(type_name);
        self.ctx
            .get_shader_generator()
            .get_syntax()
            .get_syntax()
            .get_default_value(&td, as_uniform)
    }
    fn get_gen_options(&self) -> &crate::gen_shader::GenOptions {
        self.ctx.get_options()
    }
    fn as_graph_create_context(&self) -> Option<&dyn ShaderGraphCreateContext> {
        Some(self)
    }
}

fn mdl_type_for_emit(type_name: &str) -> (&'static str, &'static str) {
    match type_name {
        "float" => ("float", "0.0"),
        "integer" => ("int", "0"),
        "boolean" => ("bool", "false"),
        "vector2" => ("float2", "float2(0.0)"),
        "vector3" => ("float3", "float3(0.0)"),
        "vector4" => ("float4", "float4(0.0)"),
        "color3" => ("color", "color(0.0)"),
        "color4" => ("color4", "mk_color4(0.0)"),
        "matrix33" => ("float3x3", "float3x3(1.0)"),
        "matrix44" => ("float4x4", "float4x4(1.0)"),
        "string" => ("string", "\"\""),
        "filename" => ("texture_2d", "texture_2d()"),
        "surfaceshader" | "volumeshader" | "displacementshader" | "material" | "BSDF" | "EDF"
        | "VDF" | "lightshader" => ("material", "material()"),
        _ => ("float", "0.0"),
    }
}

/// Check if an implementation name matches a registered MDL-specific node.
/// Returns a creator function if matched.
/// Ref: MdlShaderGenerator constructor registration list.
fn get_mdl_node_impl(impl_name: &str) -> Option<fn() -> Box<dyn ShaderNodeImpl>> {
    match impl_name {
        // surfacematerial
        "IM_surfacematerial_genmdl" => Some(|| MaterialNodeMdl::create()),
        // surface
        "IM_surface_genmdl" => Some(|| SurfaceNodeMdl::create()),
        // heighttonormal
        "IM_heighttonormal_vector3_genmdl" => Some(|| HeightToNormalNodeMdl::create()),
        // layer (BSDF and VDF)
        "IM_layer_bsdf_genmdl" | "IM_layer_vdf_genmdl" => Some(|| ClosureLayerNodeMdl::create()),
        // layerable BSDFs
        "IM_dielectric_bsdf_genmdl"
        | "IM_generalized_schlick_bsdf_genmdl"
        | "IM_sheen_bsdf_genmdl" => Some(|| LayerableNodeMdl::create()),
        // image nodes
        "IM_image_float_genmdl"
        | "IM_image_color3_genmdl"
        | "IM_image_color4_genmdl"
        | "IM_image_vector2_genmdl"
        | "IM_image_vector3_genmdl"
        | "IM_image_vector4_genmdl" => Some(|| ImageNodeMdl::create()),
        _ => None,
    }
}

impl ShaderGraphCreateContext for MdlShaderGraphContext<'_> {
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
            let impl_name = impl_elem.borrow().get_name().to_string();
            let cat = impl_elem.borrow().get_category().to_string();

            // Check for MDL-specific registered implementations (ref: MdlShaderGenerator constructor)
            if let Some(creator) = get_mdl_node_impl(&impl_name) {
                let mut impl_ = creator();
                impl_.initialize(impl_elem, self);
                return Some(impl_);
            }

            return if cat == category::NODE_GRAPH {
                // Ref: createShaderNodeImplForNodeGraph -- closure vs non-closure
                let el = impl_elem.borrow();
                let outputs: Vec<_> = el
                    .get_children()
                    .iter()
                    .filter(|c| c.borrow().get_category() == category::OUTPUT)
                    .cloned()
                    .collect();
                let is_closure = outputs
                    .first()
                    .and_then(|o| {
                        let o_ref = o.borrow();
                        let ty = o_ref.get_type()?;
                        let td = self.get_type_system().get_type(ty);
                        Some(td.is_closure())
                    })
                    .unwrap_or(false);
                drop(el);

                if is_closure {
                    let mut compound = ClosureCompoundNodeMdl::create();
                    compound.initialize(impl_elem, self);
                    Some(compound)
                } else {
                    let mut compound = CompoundNodeMdl::create();
                    compound.initialize(impl_elem, self);
                    Some(compound)
                }
            } else if cat == category::IMPLEMENTATION {
                // Ref: createShaderNodeImplForImplementation
                // Check for custom code node: file+function or sourcecode without markers
                if CustomCodeNodeMdl::is_custom_impl_element(impl_elem) {
                    let mut impl_ = CustomCodeNodeMdl::create();
                    impl_.initialize(impl_elem, self);
                    Some(impl_)
                } else {
                    let elem = impl_elem.borrow();
                    let file_attr = elem.get_attribute_or_empty("file").to_string();
                    let source_code = elem.get_attribute_or_empty("sourcecode").to_string();
                    drop(elem);
                    if !file_attr.is_empty() || !source_code.is_empty() {
                        let mut impl_ = SourceCodeNodeMdl::create();
                        impl_.initialize(impl_elem, self);
                        Some(impl_)
                    } else {
                        continue;
                    }
                }
            } else {
                continue;
            };
        }
        None
    }
}

/// MDL-specific flag: ShaderPort flag bit for transmission IOR dependency
/// (matches C++ ShaderPortFlagMdl::TRANSMISSION_IOR_DEPENDENCY = 1u << 31).
#[allow(dead_code)]
pub const TRANSMISSION_IOR_DEPENDENCY_FLAG: u32 = 1u32 << 31;

/// Check and fix transmission IOR dependencies for MDL.
/// Ref: checkTransmissionIorDependencies in MdlShaderGenerator.cpp
#[allow(dead_code)]
pub fn check_transmission_ior_dependencies(graph: &mut ShaderGraph) -> Vec<String> {
    let graph_name = graph.get_name().to_string();
    let mut uniform_sockets: Vec<String> = Vec::new();

    let bsdf_t_nodes: Vec<String> = graph
        .get_nodes()
        .filter(|n| n.has_classification(ShaderNodeClassification::BSDF_T))
        .map(|n| n.get_name().to_string())
        .collect();

    for node_name in bsdf_t_nodes {
        let conn = graph
            .get_node(&node_name)
            .and_then(|n| n.get_input("ior"))
            .and_then(|i| i.get_connection())
            .map(|(src_node, src_out)| (src_node.to_string(), src_out.to_string()));

        let Some((src_node, src_out)) = conn else {
            continue;
        };

        // Case 1: Connection from graph input socket -- mark it uniform
        if src_node == graph_name {
            if !uniform_sockets.contains(&src_out) {
                uniform_sockets.push(src_out.clone());
            }
            if let Some(socket) = graph.node.outputs.get_mut(&src_out) {
                socket.port.set_flag(TRANSMISSION_IOR_DEPENDENCY_FLAG, true);
            }
            continue;
        }

        // Case 2: Connection from a CONSTANT node -- inline value and break
        let is_constant = graph
            .get_node(&src_node)
            .map(|n| n.has_classification(ShaderNodeClassification::CONSTANT))
            .unwrap_or(false);

        if is_constant {
            let const_value = graph
                .get_node(&src_node)
                .and_then(|n| n.get_input("value"))
                .and_then(|i| i.port.get_value())
                .cloned();

            graph.break_connection(&node_name, "ior");

            if let Some(val) = const_value {
                if let Some(ior_inp) = graph
                    .get_node_mut(&node_name)
                    .and_then(|n| n.get_input_mut("ior"))
                {
                    ior_inp.port_mut().set_value(Some(val), false);
                }
            }
            continue;
        }

        // Case 3: Varying connection -- break immediately, fall back to default IOR
        graph.break_connection(&node_name, "ior");
    }

    uniform_sockets
}

/// Create MDL shader from element (ref: MdlShaderGenerator::createShader).
pub fn create_mdl_shader(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &GenContext<MdlShaderGenerator>,
) -> Result<Shader, String> {
    let mdl_ctx = MdlShaderGraphContext::new(context);
    let graph = if element.borrow().get_category() == category::NODE_GRAPH {
        create_from_nodegraph(element, doc, &mdl_ctx)
    } else {
        create_from_element(name, element, doc, &mdl_ctx)
    }?;

    let mut shader = Shader::new(name, graph);
    shader.create_stage(shader_stage::PIXEL);
    let g = shader.get_graph();
    let graph_name = g.get_name().to_string();

    // Collect input socket data (ref: C++ inputs->add(inputSocket->getSelf()))
    struct InputData {
        td: crate::gen_shader::TypeDesc,
        var: String,
        val: Option<crate::core::Value>,
        path: String,
        geomprop: String,
        is_uniform: bool,
    }
    let mut input_adds: Vec<InputData> = Vec::new();
    for i in 0..g.num_input_sockets() {
        let socket = g.get_input_socket_at(i).ok_or("Input socket")?;
        let ty = socket.port.get_type();
        // Skip shader/closure/material semantic inputs (filtered to let block)
        if ty.is_closure()
            || ty.get_semantic() == Semantic::Shader
            || ty.get_semantic() == Semantic::Material
        {
            continue;
        }
        if !g
            .get_connections_for_output(&graph_name, socket.get_name())
            .is_empty()
            && g.is_editable(socket.get_name())
        {
            input_adds.push(InputData {
                td: socket.port.get_type().clone(),
                var: socket.port.get_variable().to_string(),
                val: socket.port.get_value().cloned(),
                path: socket.port.path.clone(),
                geomprop: socket.port.geomprop.clone(),
                is_uniform: socket.port.is_uniform(),
            });
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
    stage.create_input_block(mdl_block::INPUTS, mdl_block::INPUTS);
    stage.create_output_block(mdl_block::OUTPUTS, mdl_block::OUTPUTS);

    let inputs = stage.get_input_block_mut(mdl_block::INPUTS).unwrap();
    for inp in input_adds {
        let port = inputs.add(inp.td, inp.var, inp.val, false);
        port.path = inp.path;
        port.geomprop = inp.geomprop;
        if inp.is_uniform {
            port.set_uniform(true);
        }
    }
    let outputs = stage.get_output_block_mut(mdl_block::OUTPUTS).unwrap();
    for (td, var, val) in output_adds {
        outputs.add(td, var, val, false);
    }

    Ok(shader)
}

/// Get upstream result for an MDL input -- handles multi-output nodes.
/// Ref: MdlShaderGenerator::getUpstreamResult
pub fn get_upstream_result_mdl(
    _input_name: &str,
    connection: Option<(&str, &str)>,
    graph: &ShaderGraph,
    syntax: &MdlSyntax,
) -> Option<String> {
    let (up_node_name, up_out_name) = connection?;

    // If upstream is a graph interface, use default resolution
    let up_node = graph.get_node(up_node_name)?;

    // Check for multi-output upstream node
    let output_count = up_node.get_outputs().count();
    if output_count > 1 {
        // Check if it's a CompoundNodeMdl with unrolled members
        // Pattern: node__field for unrolled, node_result.mxp_field for struct
        let _impl_name = up_node.get_impl_name().unwrap_or("").to_string();

        // For compound nodes that unroll struct members: use node__field pattern
        // For others: use node_result.mxp_field pattern
        // We detect unrolled compounds by checking if they produce shader-semantic outputs
        let has_shader_output = up_node
            .get_outputs()
            .any(|o| o.get_type().is_closure() || o.get_type().get_semantic() == Semantic::Shader);

        if has_shader_output {
            // Unrolled compound: node__field
            return Some(format!("{}__{}", up_node_name, up_out_name));
        } else {
            // Struct compound or custom node: node_result.mxp_field
            return Some(format!(
                "{}_result.{}",
                up_node_name,
                syntax.modify_port_name(up_out_name)
            ));
        }
    }

    // Single output: use the output variable directly
    graph.get_connection_variable(up_node_name, up_out_name)
}
