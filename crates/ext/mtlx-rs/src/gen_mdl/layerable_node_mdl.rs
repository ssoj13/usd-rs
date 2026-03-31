//! LayerableNodeMdl -- MDL layerable BSDF node (ref: MaterialXGenMdl/Nodes/ClosureLayerNodeMdl.cpp).
//!
//! Because MDL does not support vertical layering, layerable BSDF nodes
//! (dielectric_bsdf, generalized_schlick_bsdf, sheen_bsdf) are transformed
//! so the base node is passed as a parameter to the top layer node.
//!
//! This node extends SourceCodeNodeMdl and adds:
//! - `base` input (BSDF type) for base layer nesting
//! - `top_weight` input (float, default 1.0) for mix amount forwarding
//! Both inputs are hidden from the user interface (not editable).

use super::closure_layer_node_mdl::port;
use crate::core::ElementPtr;
use crate::gen_shader::{
    ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, type_desc_types,
};

/// MDL layerable BSDF node -- adds base and top_weight inputs for closure nesting.
/// Ref: LayerableNodeMdl in ClosureLayerNodeMdl.h/.cpp
#[derive(Debug, Default)]
pub struct LayerableNodeMdl {
    /// Inner source code node for delegation
    base_impl: super::source_code_node_mdl::SourceCodeNodeMdl,
}

impl LayerableNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }
}

impl ShaderNodeImpl for LayerableNodeMdl {
    fn get_name(&self) -> &str {
        self.base_impl.get_name()
    }

    fn get_hash(&self) -> u64 {
        self.base_impl.get_hash()
    }

    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        self.base_impl.initialize(element, context);
    }

    fn add_inputs(&self, node: &mut ShaderNode, _context: &dyn ShaderImplContext) {
        // Add the input to hold base layer BSDF (ref: LayerableNodeMdl::addInputs)
        node.add_input(port::BASE, type_desc_types::bsdf());

        // Set the top level weight default to 1.0 (ref: LayerableNodeMdl::addInputs)
        let top_weight_inp = node.add_input(port::TOP_WEIGHT, type_desc_types::float());
        top_weight_inp
            .port_mut()
            .set_value(Some(crate::core::Value::Float(1.0)), false);
    }

    fn is_editable(&self, input_name: &str) -> bool {
        // base and top_weight are not user-editable (ref: LayerableNodeMdl::isEditable)
        if input_name == port::BASE || input_name == port::TOP_WEIGHT {
            return false;
        }
        self.base_impl.is_editable(input_name)
    }

    fn emit_function_definition(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        self.base_impl
            .emit_function_definition(node, context, stage);
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        self.base_impl.emit_function_call(node, context, stage);
    }
}
