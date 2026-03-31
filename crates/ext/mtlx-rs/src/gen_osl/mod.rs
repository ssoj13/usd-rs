//! MaterialXGenOsl — OSL (Open Shading Language) shader generation.

mod nodes;
mod osl_emit;
mod osl_network_emit;
mod osl_network_shader_generator;
mod osl_network_syntax;
mod osl_shader_generator;
mod osl_syntax;

pub use nodes::OsoNode;
pub use osl_emit::generate as generate_osl_shader;
pub use osl_network_emit::generate as generate_osl_network;
pub use osl_network_shader_generator::{
    OslNetworkShaderGenerator, OslNetworkShaderGraphContext, TARGET as NETWORK_TARGET,
    create_osl_network_shader,
};
pub use osl_network_syntax::OslNetworkSyntax;
pub use osl_shader_generator::{
    OslShaderGenerator, OslShaderGraphContext, TARGET, create_osl_shader, osl_block,
    register_osl_shader_metadata,
};
pub use osl_syntax::{OUTPUT_QUALIFIER, OslSyntax, SOURCE_FILE_EXTENSION};
