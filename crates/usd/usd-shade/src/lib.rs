//! USD Shade Module (usdShade)
//!
//! This module provides shading schemas for USD, including materials, shaders,
//! node graphs, and their connections.

pub mod connectable_api;
pub mod connectable_api_behavior;
pub mod coord_sys_api;
pub mod input;
pub mod material;
pub mod material_binding_api;
pub mod node_def_api;
pub mod node_graph;
pub mod output;
pub mod shader;
pub mod shader_def_parser;
pub mod shader_def_utils;
pub mod tokens;
pub mod types;
pub mod udim_utils;
pub mod utils;

pub use connectable_api::{ConnectableAPI, ConnectionSourceInfo};
pub use coord_sys_api::{Binding, CoordSysAPI};
pub use input::Input;
pub use material::Material;
pub use material_binding_api::{
    BindingsAtPrim, BindingsCache, CollectionBinding, CollectionBindingVector,
    CollectionMembershipQuery, CollectionQueryCache, DirectBinding, MaterialBindingAPI,
};
pub use node_def_api::NodeDefAPI;
pub use node_graph::NodeGraph;
pub use output::Output;
pub use shader::Shader;
pub use shader_def_parser::{
    ShaderDefParseResult, get_discovery_types, get_source_type, parse_shader_node,
};
pub use shader_def_utils::{
    ShaderPropertyInfo, get_primvar_names_metadata_string, get_properties,
    get_sdr_property_type_and_array_size, get_source_asset,
};
pub use tokens::{UsdShadeTokens, tokens};
pub use types::{
    AttributeType, AttributeVector, ConnectionModification, SdrTokenMap, SourceInfoVector,
};
pub use udim_utils::{
    ResolvedPathAndTile, is_udim_identifier, replace_udim_pattern, resolve_udim_path,
    resolve_udim_tile_paths,
};
pub use utils::Utils;
