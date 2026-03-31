//! CompoundNode — NodeGraph as ShaderNodeImpl (по рефу MaterialX CompoundNode).

use crate::core::element::category;
use crate::core::{
    Document, ElementPtr, get_active_inputs, get_active_outputs, get_node_def_string,
    traverse_graph,
};

use super::Shader;
use super::ShaderStage;
use super::gen_context::ShaderImplContext;
use super::gen_options::{GenOptions, ShaderInterfaceType};
use super::shader_graph::ShaderGraph;
use super::shader_graph_create::{ShaderGraphCreateContext, create_from_nodegraph};
use super::shader_node::ShaderNode;
use super::shader_node_impl::ShaderNodeImpl;
use super::util::hash_string;

/// Compound node — wraps a NodeGraph as expandable shader graph.
#[derive(Debug)]
pub struct CompoundNode {
    name: String,
    hash: u64,
    root_graph: Option<ShaderGraph>,
    function_name: String,
    document: Option<Document>,
}

impl CompoundNode {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            hash: 0,
            root_graph: None,
            function_name: String::new(),
            document: None,
        }
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    fn resolve_child_impl(
        &self,
        graph: &ShaderGraph,
        node_name: &str,
        context: &dyn ShaderImplContext,
    ) -> Option<Box<dyn ShaderNodeImpl>> {
        let doc = self.document.as_ref()?;
        let graph_ctx = context.as_graph_create_context()?;
        let node_def_name = graph.get_node_def(node_name)?;
        graph_ctx.get_implementation_for_nodedef(
            doc,
            node_def_name,
            graph_ctx.get_implementation_target(),
        )
    }

    fn emit_child_function_definitions(
        &self,
        graph: &ShaderGraph,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        for node_name in &graph.node_order {
            let Some(child) = graph.get_node(node_name) else {
                continue;
            };
            let Some(impl_) = self.resolve_child_impl(graph, node_name, context) else {
                continue;
            };
            impl_.emit_function_definition(child, context, stage);
        }
    }

    fn emit_child_function_call(
        &self,
        graph: &ShaderGraph,
        node_name: &str,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.is_function_call_emitted(node_name) {
            return;
        }
        let Some(child) = graph.get_node(node_name) else {
            return;
        };
        let Some(impl_) = self.resolve_child_impl(graph, node_name, context) else {
            return;
        };
        impl_.emit_function_call(child, context, stage);
        stage.add_function_call_emitted(node_name.to_string());
    }

    fn emit_child_function_calls(
        &self,
        graph: &ShaderGraph,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
        classification_mask: Option<u32>,
    ) {
        for node_name in &graph.node_order {
            let Some(child) = graph.get_node(node_name) else {
                continue;
            };
            if let Some(mask) = classification_mask {
                if child.classification & mask == 0 {
                    continue;
                }
            }
            self.emit_child_function_call(graph, node_name, context, stage);
        }
    }

    fn emit_dependent_function_calls(
        &self,
        node: &ShaderNode,
        graph: &ShaderGraph,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
        classification_mask: u32,
    ) {
        for input in node.get_inputs() {
            let Some((up_node_name, _up_output_name)) = input.get_connection() else {
                continue;
            };
            let Some(upstream) = graph.get_node(up_node_name) else {
                continue;
            };
            self.emit_dependent_function_calls(
                upstream,
                graph,
                context,
                stage,
                classification_mask,
            );
            if upstream.classification & classification_mask != 0 {
                self.emit_child_function_call(graph, up_node_name, context, stage);
            }
        }
    }

    fn resolve_input_arg(
        &self,
        input: &super::shader_node::ShaderInput,
        context: &dyn ShaderImplContext,
    ) -> String {
        if let Some((up_node, up_output)) = input.get_connection() {
            if let Some(graph) = context.get_graph() {
                if let Some(var) = graph.get_connection_variable(up_node, up_output) {
                    let type_name = input.port.get_type().get_name();
                    if type_name == "filename" {
                        return context.format_filename_arg(&var);
                    }
                    return var;
                }
            }
        }
        let val = input.port().get_value_string();
        if !val.is_empty() {
            return val;
        }
        let type_name = input.port.get_type().get_name();
        context.get_default_value(type_name, false)
    }

    fn output_type_name(&self, type_name: &str, context: &dyn ShaderImplContext) -> String {
        context
            .get_type_name(type_name)
            .unwrap_or_else(|| type_name.to_string())
    }
}

impl Default for CompoundNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderNodeImpl for CompoundNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        if element.borrow().get_category() != category::NODE_GRAPH {
            return;
        }
        self.name = element.borrow().get_name().to_string();
        self.function_name = self.name.clone();
        context.make_valid_name(&mut self.function_name);
        self.document = Document::from_element(element);
        if let Some(doc) = Document::from_element(element) {
            if let Some(graph) = create_shader_graph_from_nodegraph(element, &doc, context) {
                self.root_graph = Some(graph);
            }
        }
        self.hash = hash_string(&self.function_name);
    }

    fn add_classification(&self, node: &mut ShaderNode) {
        if let Some(ref graph) = self.root_graph {
            node.add_classification(graph.node.classification);
        }
    }

    fn get_graph(&self) -> Option<&ShaderGraph> {
        self.root_graph.as_ref()
    }

    fn create_variables(
        &self,
        _node_name: &str,
        context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        let Some(graph) = self.root_graph.as_ref() else {
            return;
        };
        let emit_ctx = CompoundEmitContext::new(context, graph);
        for node_name in &graph.node_order {
            let Some(impl_) = self.resolve_child_impl(graph, node_name, &emit_ctx) else {
                continue;
            };
            impl_.create_variables(node_name, &emit_ctx, shader);
        }
    }

    fn emit_function_definition(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != super::shader::stage::PIXEL {
            return;
        }
        let Some(graph) = self.root_graph.as_ref() else {
            return;
        };
        let emit_ctx = CompoundEmitContext::new(context, graph);

        stage.add_function_definition_by_hash(self.hash, |stage| {
            self.emit_child_function_definitions(graph, &emit_ctx, stage);

            let mut params = Vec::new();
            if let Some(param) = context.get_closure_data_parameter(node) {
                params.push(param);
            }
            for input_socket in
                (0..graph.num_input_sockets()).filter_map(|idx| graph.get_input_socket_at(idx))
            {
                let type_name =
                    self.output_type_name(input_socket.get_type().get_name(), &emit_ctx);
                params.push(format!(
                    "{} {}",
                    type_name,
                    input_socket.port.get_variable()
                ));
            }
            for output_socket in
                (0..graph.num_output_sockets()).filter_map(|idx| graph.get_output_socket_at(idx))
            {
                let type_name =
                    self.output_type_name(output_socket.get_type().get_name(), &emit_ctx);
                params.push(format!(
                    "out {} {}",
                    type_name,
                    output_socket.port.get_variable()
                ));
            }

            stage.append_line(&format!(
                "void {}({})",
                self.function_name,
                params.join(", ")
            ));
            stage.append_line("{");

            if self.node_output_is_closure(node) {
                self.emit_child_function_calls(
                    graph,
                    &emit_ctx,
                    stage,
                    Some(super::shader_node::ShaderNodeClassification::TEXTURE),
                );
                for output_socket in (0..graph.num_output_sockets())
                    .filter_map(|idx| graph.get_output_socket_at(idx))
                {
                    let Some((up_node, _up_output)) = output_socket.get_connection() else {
                        continue;
                    };
                    let Some(upstream) = graph.get_node(up_node) else {
                        continue;
                    };
                    let closure_mask = super::shader_node::ShaderNodeClassification::CLOSURE
                        | super::shader_node::ShaderNodeClassification::SHADER
                        | super::shader_node::ShaderNodeClassification::MATERIAL;
                    if upstream.classification & closure_mask != 0 {
                        self.emit_child_function_call(graph, up_node, &emit_ctx, stage);
                    }
                }
            } else {
                self.emit_child_function_calls(graph, &emit_ctx, stage, None);
            }

            for output_socket in
                (0..graph.num_output_sockets()).filter_map(|idx| graph.get_output_socket_at(idx))
            {
                let Some((up_node, up_output)) = output_socket.get_connection() else {
                    continue;
                };
                let Some(result) = graph.get_connection_variable(up_node, up_output) else {
                    continue;
                };
                stage.append_line(&format!(
                    "{} = {};",
                    output_socket.port.get_variable(),
                    result
                ));
            }

            stage.append_line("}");
            stage.append_line("");
        });
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let Some(graph) = self.root_graph.as_ref() else {
            return;
        };
        let emit_ctx = CompoundEmitContext::new(context, graph);

        if stage.get_name() == super::shader::stage::VERTEX {
            self.emit_child_function_calls(graph, &emit_ctx, stage, None);
            return;
        }
        if stage.get_name() != super::shader::stage::PIXEL {
            return;
        }

        if self.node_output_is_closure(node) {
            if let Some(outer_graph) = context.get_graph() {
                self.emit_dependent_function_calls(
                    node,
                    outer_graph,
                    context,
                    stage,
                    super::shader_node::ShaderNodeClassification::CLOSURE,
                );
            }
        }

        self.emit_output_variables(node, context, stage);

        let mut args = Vec::new();
        if let Some(arg) = context.get_closure_data_argument(node) {
            args.push(arg);
        }
        for input in node.get_inputs() {
            args.push(self.resolve_input_arg(input, context));
        }
        for output in node.get_outputs() {
            args.push(output.port.get_variable().to_string());
        }
        stage.append_line(&format!("{}({});", self.function_name, args.join(", ")));
    }

    fn emit_output_variables(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        for output in node.get_outputs() {
            let type_name = self.output_type_name(output.get_type().get_name(), context);
            let default_value = context.get_default_value(output.get_type().get_name(), false);
            stage.append_line(&format!(
                "{} {} = {};",
                type_name,
                output.port.get_variable(),
                default_value
            ));
        }
    }
}

struct ReducedCompoundGraphContext<'a> {
    outer: &'a dyn ShaderGraphCreateContext,
    options: GenOptions,
}

impl<'a> ReducedCompoundGraphContext<'a> {
    fn new(outer: &'a dyn ShaderGraphCreateContext) -> Self {
        let mut options = outer.get_options().clone();
        options.shader_interface_type = ShaderInterfaceType::Reduced;
        Self { outer, options }
    }
}

impl ShaderImplContext for ReducedCompoundGraphContext<'_> {
    fn resolve_source_file(
        &self,
        filename: &str,
        local_path: Option<&crate::format::FilePath>,
    ) -> Option<crate::format::FilePath> {
        self.outer.resolve_source_file(filename, local_path)
    }

    fn get_type_system(&self) -> &super::TypeSystem {
        self.outer.get_type_system()
    }

    fn get_graph(&self) -> Option<&ShaderGraph> {
        self.outer.get_graph()
    }

    fn get_gen_options(&self) -> &GenOptions {
        &self.options
    }

    fn format_filename_arg(&self, var: &str) -> String {
        self.outer.format_filename_arg(var)
    }

    fn get_type_name_for_emit(&self, type_name: &str) -> Option<(&'static str, &'static str)> {
        self.outer.get_type_name_for_emit(type_name)
    }

    fn get_reserved_words(&self) -> Option<&std::collections::HashSet<String>> {
        self.outer.get_reserved_words()
    }

    fn make_valid_name(&self, name: &mut String) {
        self.outer.make_valid_name(name);
    }

    fn get_constant_qualifier(&self) -> &str {
        self.outer.get_constant_qualifier()
    }

    fn get_closure_data_argument(&self, node: &ShaderNode) -> Option<String> {
        self.outer.get_closure_data_argument(node)
    }

    fn get_substitution_tokens(&self) -> Vec<(String, String)> {
        self.outer.get_substitution_tokens()
    }

    fn get_file_texture_vertical_flip(&self) -> bool {
        self.outer.get_file_texture_vertical_flip()
    }

    fn get_mdl_version_suffix(&self) -> &str {
        self.outer.get_mdl_version_suffix()
    }

    fn get_default_value(&self, type_name: &str, as_uniform: bool) -> String {
        self.outer.get_default_value(type_name, as_uniform)
    }

    fn as_graph_create_context(&self) -> Option<&dyn ShaderGraphCreateContext> {
        Some(self)
    }
}

impl ShaderGraphCreateContext for ReducedCompoundGraphContext<'_> {
    fn get_syntax(&self) -> &super::syntax::Syntax {
        self.outer.get_syntax()
    }

    fn get_options(&self) -> &GenOptions {
        &self.options
    }

    fn get_implementation_for_nodedef(
        &self,
        doc: &Document,
        node_def_name: &str,
        target: &str,
    ) -> Option<Box<dyn ShaderNodeImpl>> {
        self.outer
            .get_implementation_for_nodedef(doc, node_def_name, target)
    }

    fn get_target(&self) -> &str {
        self.outer.get_target()
    }

    fn get_implementation_target(&self) -> &str {
        self.outer.get_implementation_target()
    }

    fn get_color_management_system(&self) -> Option<&dyn super::ColorManagementSystem> {
        self.outer.get_color_management_system()
    }

    fn get_unit_system(&self) -> Option<&dyn super::UnitSystem> {
        self.outer.get_unit_system()
    }

    fn get_shader_metadata_registry(&self) -> Option<&super::ShaderMetadataRegistry> {
        self.outer.get_shader_metadata_registry()
    }
}

struct CompoundEmitContext<'a> {
    outer: &'a dyn ShaderImplContext,
    graph_create_context: Option<&'a dyn ShaderGraphCreateContext>,
    graph: &'a ShaderGraph,
}

impl<'a> CompoundEmitContext<'a> {
    fn new(outer: &'a dyn ShaderImplContext, graph: &'a ShaderGraph) -> Self {
        Self {
            outer,
            graph_create_context: outer.as_graph_create_context(),
            graph,
        }
    }
}

impl ShaderImplContext for CompoundEmitContext<'_> {
    fn resolve_source_file(
        &self,
        filename: &str,
        local_path: Option<&crate::format::FilePath>,
    ) -> Option<crate::format::FilePath> {
        self.outer.resolve_source_file(filename, local_path)
    }

    fn get_type_system(&self) -> &super::TypeSystem {
        self.outer.get_type_system()
    }

    fn get_graph(&self) -> Option<&ShaderGraph> {
        Some(self.graph)
    }

    fn get_gen_options(&self) -> &GenOptions {
        self.outer.get_gen_options()
    }

    fn format_filename_arg(&self, var: &str) -> String {
        self.outer.format_filename_arg(var)
    }

    fn get_type_name_for_emit(&self, type_name: &str) -> Option<(&'static str, &'static str)> {
        self.outer.get_type_name_for_emit(type_name)
    }

    fn get_reserved_words(&self) -> Option<&std::collections::HashSet<String>> {
        self.outer.get_reserved_words()
    }

    fn make_valid_name(&self, name: &mut String) {
        self.outer.make_valid_name(name);
    }

    fn get_constant_qualifier(&self) -> &str {
        self.outer.get_constant_qualifier()
    }

    fn get_closure_data_argument(&self, node: &ShaderNode) -> Option<String> {
        self.outer.get_closure_data_argument(node)
    }

    fn get_substitution_tokens(&self) -> Vec<(String, String)> {
        self.outer.get_substitution_tokens()
    }

    fn get_file_texture_vertical_flip(&self) -> bool {
        self.outer.get_file_texture_vertical_flip()
    }

    fn get_mdl_version_suffix(&self) -> &str {
        self.outer.get_mdl_version_suffix()
    }

    fn get_default_value(&self, type_name: &str, as_uniform: bool) -> String {
        self.outer.get_default_value(type_name, as_uniform)
    }

    fn as_graph_create_context(&self) -> Option<&dyn ShaderGraphCreateContext> {
        self.graph_create_context
    }
}

/// Create ShaderGraph from NodeGraph element. Public for graph_builder.
pub fn create_shader_graph_from_nodegraph(
    node_graph: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderImplContext,
) -> Option<ShaderGraph> {
    if let Some(graph_context) = context.as_graph_create_context() {
        let reduced_context = ReducedCompoundGraphContext::new(graph_context);
        if let Ok(graph) = create_from_nodegraph(node_graph, doc, &reduced_context) {
            return Some(graph);
        }
    }

    let nd_name = get_node_def_string(node_graph)?;
    let node_def = doc.get_node_def(&nd_name)?;
    let type_system = context.get_type_system();

    let graph_name = node_graph.borrow().get_name().to_string();
    let mut graph = ShaderGraph::new(&graph_name);

    // Add input sockets from NodeDef
    for input in get_active_inputs(&node_def) {
        let inp = input.borrow();
        let name = inp.get_name().to_string();
        let ty = inp
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "float".to_string());
        let type_desc = type_system.get_type(&ty);
        graph.add_input_socket(&name, type_desc);
    }

    // Add output sockets from NodeGraph
    for output in get_active_outputs(node_graph) {
        let out = output.borrow();
        let name = out.get_name().to_string();
        let ty = out
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "float".to_string());
        let type_desc = type_system.get_type(&ty);
        graph.add_output_socket(&name, type_desc);
    }

    // Collect edges from traversal and create nodes + connections
    let mut edges: Vec<(ElementPtr, ElementPtr, Option<ElementPtr>)> = vec![];
    for output in get_active_outputs(node_graph) {
        traverse_graph(&output, &mut |edge| {
            // Skip edges where upstream or downstream are missing
            if let (Some(up), Some(down)) = (
                edge.get_upstream_element().cloned(),
                edge.get_downstream_element().cloned(),
            ) {
                let conn = edge.get_connecting_element().cloned();
                edges.push((down, up, conn));
            }
        });
    }

    // Process edges: create nodes and connect
    for (downstream, upstream, connecting) in edges {
        let up_cat = upstream.borrow().get_category().to_string();
        // Skip non-node elements (backdrop, comment, etc.)
        if up_cat == category::INPUT
            || up_cat == category::OUTPUT
            || up_cat == category::BACKDROP
            || up_cat == category::COMMENT
        {
            continue;
        }
        let node_name = upstream.borrow().get_name().to_string();
        if graph.get_node(&node_name).is_some() {
            // Already created, just need to connect
        } else {
            // Create shader node for this MaterialX node.
            // Resolve NodeDef: explicit nodedef attr, else match by category + output type (по рефу Node::getNodeDef).
            let nd = upstream
                .borrow()
                .get_attribute("nodedef")
                .and_then(|s| doc.get_node_def(s))
                .or_else(|| {
                    let node_type = upstream
                        .borrow()
                        .get_type()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "float".to_string());
                    for candidate in doc.get_matching_node_defs(&up_cat) {
                        let nd_out_type = get_active_outputs(&candidate)
                            .first()
                            .and_then(|o| o.borrow().get_type().map(|s| s.to_string()))
                            .unwrap_or_else(|| "float".to_string());
                        if nd_out_type == node_type {
                            return Some(candidate);
                        }
                    }
                    doc.get_node_def(&format!("ND_{}_{}", up_cat, node_type))
                })?;

            let mut shader_node = ShaderNode::new(&node_name);
            for inp in get_active_inputs(&nd) {
                let name = inp.borrow().get_name().to_string();
                let ty = inp
                    .borrow()
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "float".to_string());
                shader_node.add_input(name, type_system.get_type(&ty));
            }
            for out in get_active_outputs(&nd) {
                let name = out.borrow().get_name().to_string();
                let ty = out
                    .borrow()
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "float".to_string());
                shader_node.add_output(name, type_system.get_type(&ty));
            }
            graph.add_node(shader_node);
        }

        // Make connection
        let down_cat = downstream.borrow().get_category().to_string();
        let up_name = upstream.borrow().get_name().to_string();
        let out_name = connecting
            .as_ref()
            .and_then(|c| {
                c.borrow()
                    .get_attribute(crate::core::element::OUTPUT_ATTRIBUTE)
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "out".to_string());

        if down_cat == category::OUTPUT {
            // Connect graph output socket (root node input) to upstream node output
            let out_socket_name = downstream.borrow().get_name().to_string();
            let graph_name = graph.get_name().to_string();
            let _ = graph.make_connection(&graph_name, &out_socket_name, &up_name, &out_name);
        } else if down_cat != category::INPUT {
            let down_name = downstream.borrow().get_name().to_string();
            let conn_input = connecting
                .as_ref()
                .map(|c| c.borrow().get_name().to_string());
            let input_name = conn_input.as_deref().unwrap_or("in1");
            let _ = graph.make_connection(&down_name, input_name, &up_name, &out_name);
        }
    }

    // Set variable names using the graph's own method (consolidated, no duplication).
    let syntax = crate::gen_shader::Syntax::new(crate::gen_shader::TypeSystem::new());
    graph.set_variable_names(&syntax);

    Some(graph)
}
