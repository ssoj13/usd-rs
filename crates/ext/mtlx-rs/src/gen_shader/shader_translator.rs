//! ShaderTranslator -- translate between shading models (ref: MaterialX ShaderTranslator).
//!
//! C++ ref: source/MaterialXGenShader/ShaderTranslator.h/cpp
//!
//! The translator looks up a "translation nodegraph" in the document for the pair
//! (source_shader_category, dest_shader_category), e.g. "standard_surface" -> "gltf_pbr".
//! It then rewires the material's connections through that nodegraph.

use std::collections::HashSet;

use crate::core::element::{
    ElementPtr, NODE_GRAPH_ATTRIBUTE, OUTPUT_ATTRIBUTE, VALUE_ATTRIBUTE, category,
};
use crate::core::{
    Document, add_child_of_category, get_active_color_space, get_active_input, get_active_outputs,
    get_connected_output, get_connected_outputs, get_input, get_inputs, get_node_def, get_output,
    get_outputs, get_shader_nodes, set_connected_output as set_interface_connected_output,
};

use super::{ShaderGraph, connects_to_world_space_node, find_renderable_material_nodes};

/// Result type for translation operations.
pub type TranslateResult = Result<(), TranslateError>;

/// Errors that can occur during shader translation.
#[derive(Debug, Clone)]
pub enum TranslateError {
    /// No translation nodegraph found for the source->dest pair.
    NoTranslation { source: String, dest: String },
    /// Source shader element is invalid or missing.
    InvalidShader(String),
}

impl std::fmt::Display for TranslateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTranslation { source, dest } => {
                write!(f, "No translation from '{}' to '{}'", source, dest)
            }
            Self::InvalidShader(msg) => write!(f, "Invalid shader element: {}", msg),
        }
    }
}

impl std::error::Error for TranslateError {}

/// Translator between shading models (e.g. standard_surface -> gltf_pbr).
///
/// C++ ref: ShaderTranslator::translateShader / translateAllMaterials.
/// Finds a translation nodedef (named "<src>_to_<dest>") in the document,
/// instantiates the translation node in a graph, and rewires the shader's
/// inputs through the translation node's outputs.
#[derive(Debug, Default)]
pub struct ShaderTranslator {
    /// Working graph for the current translation operation.
    graph: Option<ElementPtr>,
    /// Translation node instance.
    translation_node: Option<ElementPtr>,
}

impl ShaderTranslator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Self {
        Self::new()
    }

    /// Connect translation node inputs from the original shader's inputs.
    /// Mirrors C++ ShaderTranslator::connectTranslationInputs.
    fn connect_translation_inputs(
        &self,
        shader: &ElementPtr,
        translation_nodedef: &ElementPtr,
    ) -> TranslateResult {
        let translation_node = self
            .translation_node
            .clone()
            .ok_or_else(|| TranslateError::InvalidShader("Missing translation node".into()))?;
        let graph = self
            .graph
            .clone()
            .ok_or_else(|| TranslateError::InvalidShader("Missing translation graph".into()))?;

        // Snapshot original inputs before we modify them.
        let orig_inputs = get_inputs(shader);
        let mut outputs_to_remove: Vec<ElementPtr> = Vec::new();

        for shader_input in &orig_inputs {
            let inp_name = shader_input.borrow().get_name().to_string();
            let inp_type = shader_input
                .borrow()
                .get_type()
                .unwrap_or("float")
                .to_string();

            // Only process inputs that exist in the translation nodedef.
            if get_active_input(translation_nodedef, &inp_name).is_none() {
                continue;
            }

            // Create corresponding input on the translation node.
            let trans_inp = if let Some(existing) = get_input(&translation_node, &inp_name) {
                existing
            } else {
                add_child_of_category(&translation_node, category::INPUT, &inp_name)
                    .map_err(TranslateError::InvalidShader)?
            };
            trans_inp.borrow_mut().set_type(&inp_type);

            if let Some(connected_output) = get_connected_output(shader_input) {
                let mut connected_node =
                    connected_output.borrow().get_parent().ok_or_else(|| {
                        TranslateError::InvalidShader(format!(
                            "Connected output '{}' has no parent node",
                            connected_output.borrow().get_name()
                        ))
                    })?;

                if let Some(world_space_node) = connects_to_world_space_node(&connected_output) {
                    if let Some(world_space_input) = get_input(&world_space_node, "in") {
                        if let Some(upstream_output) = get_connected_output(&world_space_input) {
                            if let Some(upstream_node) = upstream_output.borrow().get_parent() {
                                connected_node = upstream_node;
                            }
                        }
                    }
                }

                trans_inp
                    .borrow_mut()
                    .set_node_name(connected_node.borrow().get_name());
                trans_inp.borrow_mut().remove_attribute(OUTPUT_ATTRIBUTE);
                trans_inp
                    .borrow_mut()
                    .remove_attribute(NODE_GRAPH_ATTRIBUTE);
                trans_inp.borrow_mut().remove_attribute(VALUE_ATTRIBUTE);
                outputs_to_remove.push(connected_output);
            } else if shader_input.borrow().has_value_string() {
                trans_inp
                    .borrow_mut()
                    .set_value_string(shader_input.borrow().get_value_string());
            } else {
                return Err(TranslateError::InvalidShader(format!(
                    "Shader input has no associated output or value {}",
                    inp_name
                )));
            }

            let color_space = get_active_color_space(shader_input);
            if !color_space.is_empty() {
                trans_inp.borrow_mut().set_color_space(color_space);
            }
            if shader_input.borrow().has_unit() {
                if let Some(unit) = shader_input.borrow().get_unit().map(|s| s.to_string()) {
                    trans_inp.borrow_mut().set_unit(unit);
                }
                if let Some(unit_type) =
                    shader_input.borrow().get_unit_type().map(|s| s.to_string())
                {
                    trans_inp.borrow_mut().set_unit_type(unit_type);
                }
            }
        }

        // Remove original inputs from the shader.
        for inp in &orig_inputs {
            let name = inp.borrow().get_name().to_string();
            shader.borrow_mut().remove_child(&name);
        }

        // Remove referenced outputs from the graph.
        let mut removed_outputs = HashSet::new();
        for output in &outputs_to_remove {
            let out_name = output.borrow().get_name().to_string();
            if removed_outputs.insert(out_name.clone()) {
                graph.borrow_mut().remove_child(&out_name);
            }
        }
        Ok(())
    }

    /// Connect translation node outputs to the shader's new inputs.
    /// Mirrors C++ ShaderTranslator::connectTranslationOutputs.
    fn connect_translation_outputs(&self, shader: &ElementPtr) -> TranslateResult {
        let translation_node = self
            .translation_node
            .clone()
            .ok_or_else(|| TranslateError::InvalidShader("Missing translation node".into()))?;
        let graph = self
            .graph
            .clone()
            .ok_or_else(|| TranslateError::InvalidShader("Missing translation graph".into()))?;

        // Get the nodedef for the translation node to iterate its outputs.
        let nd = get_node_def(&translation_node, "", true).ok_or_else(|| {
            TranslateError::InvalidShader(format!(
                "No nodedef for {} was found",
                translation_node.borrow().get_category()
            ))
        })?;

        for trans_output in get_active_outputs(&nd) {
            let output_name = trans_output.borrow().get_name().to_string();
            let output_type = trans_output
                .borrow()
                .get_type()
                .unwrap_or("float")
                .to_string();

            // Convert output name to input name: strip "_out" suffix.
            let input_name = output_name
                .find("_out")
                .map(|pos| output_name[..pos].to_string())
                .ok_or_else(|| {
                    TranslateError::InvalidShader(format!(
                        "Translation graph output {} does not end with '_out'",
                        output_name
                    ))
                })?;

            let mut translated_stream_node = translation_node.clone();
            let mut translated_stream_output = Some(output_name.clone());

            if let Some(world_space_node) = connects_to_world_space_node(&trans_output) {
                if let Some(node_input) = get_input(&world_space_node, "in") {
                    if node_input.borrow().has_interface_name() {
                        let interface_input =
                            get_input(&translation_node, &node_input.borrow().get_interface_name());
                        let Some(interface_input) = interface_input else {
                            continue;
                        };
                        let Some(source_output) = get_connected_output(&interface_input) else {
                            continue;
                        };
                        let Some(source_node) = source_output.borrow().get_parent() else {
                            continue;
                        };

                        let world_space_name = world_space_node.borrow().get_name().to_string();
                        translated_stream_node = add_child_of_category(
                            &graph,
                            world_space_node.borrow().get_category(),
                            &world_space_name,
                        )
                        .map_err(TranslateError::InvalidShader)?;
                        if let Some(node_type) =
                            world_space_node.borrow().get_type().map(|s| s.to_string())
                        {
                            translated_stream_node.borrow_mut().set_type(node_type);
                        }
                        let translated_input = if let Some(existing) =
                            get_input(&translated_stream_node, "in")
                        {
                            existing
                        } else {
                            add_child_of_category(&translated_stream_node, category::INPUT, "in")
                                .map_err(TranslateError::InvalidShader)?
                        };
                        if let Some(input_type) =
                            node_input.borrow().get_type().map(|s| s.to_string())
                        {
                            translated_input.borrow_mut().set_type(input_type);
                        }
                        translated_input
                            .borrow_mut()
                            .set_node_name(source_node.borrow().get_name());
                        translated_input
                            .borrow_mut()
                            .remove_attribute(OUTPUT_ATTRIBUTE);
                        translated_input
                            .borrow_mut()
                            .remove_attribute(NODE_GRAPH_ATTRIBUTE);
                        translated_input
                            .borrow_mut()
                            .remove_attribute(VALUE_ATTRIBUTE);
                        translated_stream_output = None;
                    }
                }
            }

            // Create/get the translated output in the graph.
            let translated_output = if let Some(existing) = get_output(&graph, &output_name) {
                existing
            } else if let Ok(new_out) =
                add_child_of_category(&graph, category::OUTPUT, &output_name)
            {
                new_out.borrow_mut().set_type(&output_type);
                new_out
            } else {
                continue;
            };

            // Connect the output to the translation node.
            translated_output
                .borrow_mut()
                .set_node_name(translated_stream_node.borrow().get_name());
            translated_output
                .borrow_mut()
                .remove_attribute(NODE_GRAPH_ATTRIBUTE);
            if let Some(stream_output) = translated_stream_output {
                translated_output
                    .borrow_mut()
                    .set_output_string(stream_output);
            } else {
                translated_output
                    .borrow_mut()
                    .remove_attribute(OUTPUT_ATTRIBUTE);
            }

            let translated_input =
                set_interface_connected_output(shader, &input_name, Some(&translated_output))
                    .map_err(TranslateError::InvalidShader)?;
            translated_input.borrow_mut().set_type(&output_type);
        }
        Ok(())
    }

    /// Translate a single shader node element to the destination shading model category.
    ///
    /// C++ ref: ShaderTranslator::translateShader(NodePtr shader, string destCategory).
    ///
    /// Looks up a translation nodedef named "<src>_to_<dest>" in the document,
    /// instantiates it in a graph, rewires the shader's inputs through the translation
    /// node, and changes the shader's category to the destination.
    pub fn translate_shader(
        &mut self,
        shader: &ElementPtr,
        dest_category: &str,
    ) -> TranslateResult {
        self.graph = None;
        self.translation_node = None;

        let src_category = shader.borrow().get_category().to_string();
        if src_category == dest_category {
            return Err(TranslateError::InvalidShader(format!(
                "The source shader \"{}\" category is already \"{}\"",
                shader.borrow().get_name_path(None),
                dest_category
            )));
        }

        // Get the document from the shader element.
        let mut doc = Document::from_element(shader)
            .ok_or_else(|| TranslateError::InvalidShader("Cannot find document".into()))?;

        // Find or create the working graph.
        let referenced_outputs = get_connected_outputs(shader);
        let referenced_parent = referenced_outputs
            .first()
            .and_then(|output| output.borrow().get_parent());
        let working_graph = match referenced_parent {
            Some(parent) if parent.borrow().get_category() == category::NODE_GRAPH => parent,
            _ => {
                let root = doc.get_root();
                let name = root.borrow().create_valid_child_name("nodegraph");
                doc.add_node_graph(&name)
                    .map_err(TranslateError::InvalidShader)?
            }
        };
        self.graph = Some(working_graph.clone());

        // Look up translation nodedef: "<src>_to_<dest>"
        let translate_node_string = format!("{}_to_{}", src_category, dest_category);
        let matching = doc.get_matching_node_defs(&translate_node_string);
        if matching.is_empty() {
            return Err(TranslateError::NoTranslation {
                source: src_category,
                dest: dest_category.to_string(),
            });
        }
        let translation_nodedef = matching[0].clone();

        // Instantiate the translation node in the graph.
        let nd_name = translation_nodedef.borrow().get_name().to_string();
        let trans_name = working_graph
            .borrow()
            .create_valid_child_name(&translate_node_string);
        let trans_node = add_child_of_category(&working_graph, &translate_node_string, &trans_name)
            .map_err(|e| TranslateError::InvalidShader(e))?;
        trans_node
            .borrow_mut()
            .set_attribute(crate::core::element::NODE_DEF_ATTRIBUTE, nd_name);
        self.translation_node = Some(trans_node);

        // Rewire inputs through the translation node.
        self.connect_translation_inputs(shader, &translation_nodedef)?;

        // Change shader category to destination.
        shader.borrow_mut().set_category(dest_category);
        shader
            .borrow_mut()
            .remove_attribute(crate::core::element::NODE_DEF_ATTRIBUTE);

        // Connect translation outputs back to shader.
        self.connect_translation_outputs(shader)?;

        Ok(())
    }

    /// Translate all materials in the document to the destination shading model.
    ///
    /// C++ ref: ShaderTranslator::translateAllMaterials(DocumentPtr doc, string destShader).
    ///
    /// Iterates material nodes, finds their bound shader children (inputs with
    /// type "surfaceshader"), and translates each shader node to `dest_shader`.
    pub fn translate_all_materials(
        &mut self,
        doc: &Document,
        dest_shader: &str,
    ) -> Vec<TranslateError> {
        let mut errors = Vec::new();
        for material in find_renderable_material_nodes(doc) {
            for shader_node in get_shader_nodes(&material, "", "") {
                if let Err(err) = self.translate_shader(&shader_node, dest_shader) {
                    errors.push(err);
                }
            }
        }
        errors
    }

    /// Translate a shader graph in-place from one shading model to another.
    ///
    /// Graph-level translation: looks up a translation nodedef "<src>_to_<dest>"
    /// in `doc`. If found, inserts a translation node into the graph and rewires
    /// the root node's inputs through it. If not found, returns NoTranslation.
    pub fn translate_graph(
        &mut self,
        graph: &mut ShaderGraph,
        doc: &Document,
        src_model: &str,
        dest_model: &str,
    ) -> TranslateResult {
        if src_model == dest_model {
            return Ok(());
        }

        // Look up translation nodedef "<src>_to_<dest>" in the document.
        let translate_str = format!("{}_to_{}", src_model, dest_model);
        let matching = doc.get_matching_node_defs(&translate_str);
        if matching.is_empty() {
            return Err(TranslateError::NoTranslation {
                source: src_model.to_string(),
                dest: dest_model.to_string(),
            });
        }
        let translation_nodedef = matching[0].clone();
        let nd_name = translation_nodedef.borrow().get_name().to_string();

        // Create a translation node in the graph.
        let trans_node_name = format!("{}_xlat", translate_str);
        let mut trans_node = super::shader_node::ShaderNode::new(&trans_node_name);
        trans_node.impl_name = Some(nd_name.clone());

        // Wire the translation node: for each nodedef output with "_out" suffix,
        // find the root node's corresponding input and route it through the
        // translation node.
        let nd_outputs = get_outputs(&translation_nodedef);
        for nd_out in &nd_outputs {
            let out_name = nd_out.borrow().get_name().to_string();
            let _out_type_str = nd_out.borrow().get_type().unwrap_or("float").to_string();
            let out_type = super::type_desc::types::float();

            let _input_name = match out_name.find("_out") {
                Some(pos) => out_name[..pos].to_string(),
                None => continue,
            };

            // Add an output port on the translation node.
            trans_node.add_output(&out_name, out_type);

            // Add an input port matching the root node's input, if present.
            let nd_inputs_list = get_inputs(&translation_nodedef);
            for nd_inp in &nd_inputs_list {
                let inp_name = nd_inp.borrow().get_name().to_string();
                let _inp_type_str = nd_inp.borrow().get_type().unwrap_or("float").to_string();
                let inp_type = super::type_desc::types::float();
                trans_node.add_input(&inp_name, inp_type);
            }
        }

        // Insert translation node into the graph.
        graph.nodes.insert(trans_node_name.clone(), trans_node);
        // Insert before the root in topological order.
        graph.node_order.insert(0, trans_node_name.clone());
        graph.set_node_def(&trans_node_name, &nd_name);

        // Rename the root node to reflect the destination model.
        graph.node.name = format!("{}_{}", graph.node.name, dest_model);

        Ok(())
    }
}
