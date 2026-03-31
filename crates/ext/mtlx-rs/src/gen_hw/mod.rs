//! MaterialXGenHw -- hardware shader generator base (by ref MaterialXGenHw).

mod create_shader;
mod hw_constants;
mod hw_light_shaders;
mod hw_resource_binding_context;
mod hw_shader_generator;
pub mod nodes;

pub use create_shader::create_shader;
pub use hw_constants::attr as hw_attr;
pub use hw_constants::block as hw_block;
pub use hw_constants::constant_values as hw_constant_values;
pub use hw_constants::get_node_space;
pub use hw_constants::ident as hw_ident;
pub use hw_constants::lighting as hw_lighting;
pub use hw_constants::space as hw_space;
pub use hw_constants::token as hw_token;
pub use hw_constants::user_data as hw_user_data;
pub use hw_light_shaders::HwLightShaders;
pub use hw_resource_binding_context::HwResourceBindingContext;
pub use hw_shader_generator::{
    ClosureContextType, HwShaderGenerator, bind_light_shader, build_token_substitutions,
    unbind_light_shader, unbind_light_shaders,
};
pub use nodes::{
    HwBitangentNode, HwFrameNode, HwGeomColorNode, HwGeomPropValueNode,
    HwGeomPropValueNodeAsUniform, HwImageNode, HwLightCompoundNode, HwLightNode,
    HwLightSamplerNode, HwLightShaderNode, HwNormalNode, HwNumLightsNode, HwPositionNode,
    HwSurfaceNode, HwTangentNode, HwTexCoordNode, HwTimeNode, HwTransformNormalNode,
    HwTransformPointNode, HwTransformVectorNode, HwViewDirectionNode,
};
