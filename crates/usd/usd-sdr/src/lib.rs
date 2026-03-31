//! SDR - Shader Definition Registry module.
//!
//! Port of pxr/usd/sdr
//!
//! This module provides a registry for shader definitions, allowing discovery
//! and access to shader node information across different shader languages
//! (OSL, GLSL, Args, etc.).
//!
//! # Overview
//!
//! SDR is the Shader Definition Registry - a system for discovering, parsing,
//! and accessing shader node definitions. It provides:
//!
//! - **Discovery**: Finds shader definitions in the filesystem or other sources
//! - **Parsing**: Extracts metadata, inputs, outputs from shader source files
//! - **Registry**: Provides singleton access to all discovered shader nodes
//! - **Querying**: Constraint-based queries across all shader nodes
//!
//! # Main Types
//!
//! - [`SdrRegistry`]: Singleton registry providing access to shader nodes
//! - [`SdrShaderNode`]: Represents a shader node with properties and metadata
//! - [`SdrShaderProperty`]: Represents an input or output of a shader node
//! - [`SdrVersion`]: Version tracking for shader nodes
//! - [`SdrShaderNodeQuery`]: Constraint-based query builder
//! - [`SdrDiscoveryPlugin`]: Interface for discovery plugins
//! - [`SdrParserPlugin`]: Interface for parser plugins
//!
//! # Example Usage
//!
//! ```ignore
//! use usd_sdr::{SdrRegistry, SdrVersionFilter};
//!
//! // Get the singleton registry
//! let registry = SdrRegistry::get_instance();
//!
//! // Get all shader identifiers
//! let ids = registry.get_shader_node_identifiers(None, SdrVersionFilter::DefaultOnly);
//!
//! // Get a specific shader node
//! for id in &ids {
//!     if let Some(node) = registry.get_shader_node_by_identifier(id, &[]) {
//!         println!("Shader: {} (context: {})",
//!             node.get_name(),
//!             node.get_context().as_str());
//!         
//!         // List inputs
//!         for input_name in node.get_shader_input_names() {
//!             if let Some(input) = node.get_shader_input(input_name) {
//!                 println!("  Input: {} (type: {})",
//!                     input.get_name().as_str(),
//!                     input.get_type().as_str());
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! # Architecture
//!
//! The SDR system follows a lazy-loading pattern:
//!
//! 1. **Discovery Phase**: Discovery plugins find shader files and create
//!    `SdrShaderNodeDiscoveryResult` instances with basic metadata.
//!
//! 2. **On-Demand Parsing**: When a client requests full node information
//!    (like inputs/outputs), parser plugins parse the shader source.
//!
//! 3. **Caching**: Parsed nodes are cached in the registry for efficient
//!    subsequent access.
//!
//! # Property Types
//!
//! SDR defines standard property types that map to SDF types:
//!
//! | SDR Type | SDF Type | Description |
//! |----------|----------|-------------|
//! | int | Int | Integer |
//! | float | Float | Floating point |
//! | string | String/Asset | Text or file path |
//! | color | Color3f | RGB color |
//! | color4 | Color4f | RGBA color |
//! | point | Point3f | 3D point |
//! | normal | Normal3f | 3D normal |
//! | vector | Vector3f | 3D vector |
//! | matrix | Matrix4d | 4x4 transform |
//!
//! # Node Context
//!
//! Shader nodes declare a context indicating their role in rendering:
//!
//! - `pattern`: Pattern evaluation shaders
//! - `surface`: Surface BXDF shaders
//! - `volume`: Volume shaders
//! - `displacement`: Displacement shaders
//! - `light`: Light shaders
//! - `lightFilter`: Light filter shaders

// Core type definitions
pub mod declare;
pub mod tokens;

// Metadata types
pub mod shader_node_metadata;
pub mod shader_property_metadata;

// Metadata helpers
pub mod shader_metadata_helpers;

// Type indicator for SDF mapping
pub mod sdf_type_indicator;

// Discovery system
pub mod discovery_plugin;
pub mod discovery_result;
pub mod filesystem_discovery;
pub mod filesystem_discovery_helpers;

// Parser system
pub mod args_parser;
pub mod osl_parser;
pub mod parser_plugin;
pub mod sdrosl_parser;

// Main types
pub mod shader_node;
pub mod shader_property;

// Query system
pub mod shader_node_query;
pub mod shader_node_query_utils;

// Registry
pub mod registry;

// Built-in USD shaders
pub mod usd_shaders;

// Re-exports for convenient access
pub use declare::{
    SdrIdentifier, SdrIdentifierSet, SdrIdentifierVec, SdrOption, SdrOptionVec, SdrStringSet,
    SdrStringVec, SdrTokenMap, SdrTokenVec, SdrVersion, SdrVersionFilter,
    sdr_get_identifier_string,
};

pub use tokens::{SdrTokens, tokens};

pub use shader_node_metadata::SdrShaderNodeMetadata;
pub use shader_property_metadata::SdrShaderPropertyMetadata;

pub use shader_metadata_helpers::{
    compute_shown_if_from_page_metadata, compute_shown_if_from_property_metadata,
    create_string_from_string_vec, get_role_from_metadata, int_val, is_property_a_terminal,
    is_property_an_asset_identifier, is_truthy, option_vec_val, parse_sdf_value, string_val,
    string_vec_val, token_val, token_vec_val,
};

pub use sdf_type_indicator::SdrSdfTypeIndicator;

pub use discovery_result::{SdrShaderNodeDiscoveryResult, SdrShaderNodeDiscoveryResultVec};

pub use discovery_plugin::{
    DefaultDiscoveryPluginContext, SdrDiscoveryPlugin, SdrDiscoveryPluginContext,
    SdrDiscoveryPluginRef, SdrDiscoveryPluginRefVec,
};

pub use filesystem_discovery_helpers::{
    SdrDiscoveryUri, SdrDiscoveryUriVec, SdrParseIdentifierFn,
    discover_files as fs_helpers_discover_files,
    discover_shader_nodes as fs_helpers_discover_shader_nodes, split_shader_identifier,
};

pub use filesystem_discovery::{
    ENV_ALLOWED_EXTS, ENV_FOLLOW_SYMLINKS, ENV_SEARCH_PATHS, SdrDiscoveryFilter,
    SdrFilesystemDiscoveryPlugin,
};

pub use parser_plugin::{
    SdrParserPlugin, SdrParserPluginRef, SdrParserPluginRefVec, SdrPassthroughParserPlugin,
    get_invalid_shader_node,
};

pub use args_parser::{ARGS_DISCOVERY_TYPE, ARGS_SOURCE_TYPE, SdrArgsParserPlugin};

pub use sdrosl_parser::{SDROSL_DISCOVERY_TYPE, SDROSL_SOURCE_TYPE, SdrOslParserPlugin};

pub use osl_parser::{OSL_DISCOVERY_TYPE, OSL_SOURCE_TYPE, OSO_DISCOVERY_TYPE, OslParserPlugin};

pub use shader_property::{
    SdrShaderProperty, SdrShaderPropertyMap, SdrShaderPropertyUniquePtr,
    SdrShaderPropertyUniquePtrVec,
};

pub use shader_node::{
    ComplianceResults, SdrShaderNode, SdrShaderNodeConstPtr, SdrShaderNodeConstPtrVec,
    SdrShaderNodePtrVec, SdrShaderNodeUniquePtr, SdrShaderNodeUniquePtrVec,
    check_property_compliance,
};

pub use shader_node_query::{
    SdrShaderNodeArc, SdrShaderNodeArcVec, SdrShaderNodeFilterFn, SdrShaderNodeQuery,
    SdrShaderNodeQueryResult,
};

pub use shader_node_query_utils::{
    GroupedQueryResult, collect_all_nodes, count_nodes, flatten_grouped_results,
    group_query_results,
};

pub use registry::SdrRegistry;

pub use usd_shaders::{
    USD_DISCOVERY_TYPE, USD_SOURCE_TYPE, UsdShadersDiscoveryPlugin, UsdShadersParserPlugin,
    register_usd_shaders, register_usd_shaders_parser,
};
