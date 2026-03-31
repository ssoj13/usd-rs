//! MaterialXGenShader — shader generation infrastructure.

mod color_management;
mod compound_node;
mod gen_context;
mod gen_options;
mod gen_user_data;
mod impl_factory;
mod material_node;
mod resource_binding_context;
mod shader;
mod shader_graph;
mod shader_graph_create;
mod shader_metadata_registry;
mod shader_node;
mod shader_node_factory;
mod shader_node_impl;
mod shader_translator;
pub mod source_code_node;
mod syntax;
mod type_desc;
mod unit_system;
mod util;

pub use color_management::{
    ColorManagementSystem, ColorSpaceTransform, DefaultColorManagementSystem,
};
pub use compound_node::CompoundNode;
pub use gen_context::{GenContext, ScopedSetVariableName, ShaderGenerator, ShaderImplContext};
pub use gen_options::{
    GenOptions, HwDirectionalAlbedoMethod, HwSpecularEnvironmentMethod, HwTransmissionRenderMethod,
    ShaderInterfaceType,
};
pub use gen_user_data::GenUserData;
pub use impl_factory::{ImplementationFactory, ShaderNodeImplCreator};
pub use material_node::MaterialNode;
pub use resource_binding_context::ResourceBindingContext;
pub use shader::stage as shader_stage;
pub use shader::{
    Shader, ShaderStage, VariableBlock, add_stage_connector, add_stage_connector_block,
    add_stage_input, add_stage_output, add_stage_uniform, add_stage_uniform_with_value,
};
pub use shader_graph::{
    ShaderGraph, ShaderGraphEdge, ShaderGraphEdgeIterator, ShaderGraphInputSocket,
    ShaderGraphOutputSocket,
};
pub(crate) use shader_graph_create::add_default_geom_node;
pub use shader_graph_create::{
    ShaderGraphCreateContext, create_from_element, create_from_nodegraph,
};
pub use shader_metadata_registry::{
    ShaderMetadataEntry, ShaderMetadataRegistry, ShaderPortMetadata, osl_attr, ui_attr,
};
pub use shader_node::{
    ShaderInput, ShaderNode, ShaderNodeClassification, ShaderOutput, ShaderPort, ShaderPortFlag,
    shader_node_category,
};
pub use shader_node_impl::{NopNode, ShaderNodeImpl};
pub use shader_translator::ShaderTranslator;
pub use source_code_node::SourceCodeNode;
pub use syntax::{
    COMMA, EnumRemapMode, GlslValueFormat, IdentifierMap, OslValueFormat, SEMICOLON,
    SlangValueFormat, Syntax, TypeSyntax, channels_mapping,
};
pub use type_desc::types as type_desc_types;
pub use type_desc::{BaseType, Semantic, StructMemberDesc, TypeDesc, TypeSystem};
pub use unit_system::{DefaultUnitSystem, UnitSystem, UnitTransform, build_registry_from_document};
pub use util::{
    GAUSSIAN_KERNEL_3, GAUSSIAN_KERNEL_5, GAUSSIAN_KERNEL_7, connects_to_world_space_node,
    element_requires_shading, find_renderable_elements, find_renderable_material_nodes,
    get_node_def_input, get_udim_coordinates, get_udim_scale_and_offset, has_element_attributes,
    hash_string, is_transparent_surface, map_value_to_color, requires_implementation,
    token_substitution,
};
