//! Parser Plugin - Interface for shader node parser plugins.
//!
//! Port of pxr/usd/sdr/parserPlugin.h
//!
//! This module defines the interface for parser plugins that convert discovery
//! results into full shader node definitions.
//!
//! # Architecture
//!
//! Parser plugins take a `SdrShaderNodeDiscoveryResult` from the discovery process
//! and create a full `SdrShaderNode` instance. The parser that is selected to run
//! is decided by the registry based on the discovery result's `discoveryType` member.
//!
//! A parser plugin's `get_discovery_types()` method links discovery types to parsers.
//! If a discovery result has a `discoveryType` of 'foo', and `SomeParserPlugin` has
//! 'foo' in its `get_discovery_types()` return value, `SomeParserPlugin` will parse
//! that discovery result.
//!
//! # Source Type vs Discovery Type
//!
//! - **Discovery Type**: Links discovery results to parser plugins (e.g., file extension)
//! - **Source Type**: Umbrella type that groups related discovery types together
//!
//! For example, a plugin handling 'foo', 'bar', and 'baz' discovery types might
//! group them all under one unifying source type.

use super::declare::SdrTokenVec;
use super::discovery_result::SdrShaderNodeDiscoveryResult;
use super::shader_node::{SdrShaderNode, SdrShaderNodeUniquePtr};
use super::shader_node_metadata::SdrShaderNodeMetadata;
use usd_tf::Token;

/// Interface for parser plugins.
///
/// Parser plugins are responsible for taking a `SdrShaderNodeDiscoveryResult`
/// and generating a full `SdrShaderNode` with all properties, metadata, and
/// other information extracted from the shader source.
///
/// # Implementation Notes
///
/// To create a parser plugin:
/// 1. Implement this trait
/// 2. Register the plugin with the registry (typically via plugin system)
/// 3. Provide a plugInfo.json describing the plugin
pub trait SdrParserPlugin: Send + Sync {
    /// Takes the specified discovery result and generates a new SdrShaderNode.
    ///
    /// The node's name, source type, and family must match those from the
    /// discovery result.
    ///
    /// Returns `None` if parsing fails.
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr>;

    /// Returns the types of nodes that this plugin can parse.
    ///
    /// "Type" here is the discovery type (in the case of files, this will
    /// probably be the file extension). This type should only be used to
    /// match up a discovery result to its parser plugin; this value is not
    /// exposed in the node's API.
    fn get_discovery_types(&self) -> SdrTokenVec;

    /// Returns the source type that this parser operates on.
    ///
    /// A source type is the most general type for a node. The parser plugin is
    /// responsible for parsing all discovery results that have the types
    /// declared under `get_discovery_types()`, and those types are collectively
    /// identified as one "source type".
    fn get_source_type(&self) -> Token;

    /// Returns the name of this parser plugin for identification purposes.
    fn get_name(&self) -> &str {
        "SdrParserPlugin"
    }
}

/// A boxed parser plugin for type-erased storage.
pub type SdrParserPluginRef = Box<dyn SdrParserPlugin>;

/// A vector of parser plugin references.
pub type SdrParserPluginRefVec = Vec<SdrParserPluginRef>;

/// Gets an invalid node based on the discovery result provided.
///
/// An invalid node is a node that has no properties, but may have basic data
/// found during discovery. This is useful when parsing fails but you still
/// want to represent the node's existence.
pub fn get_invalid_shader_node(dr: &SdrShaderNodeDiscoveryResult) -> SdrShaderNodeUniquePtr {
    // Create metadata from discovery result metadata
    let metadata = SdrShaderNodeMetadata::from_token_map(&dr.metadata);

    // Create a basic invalid node
    let node = SdrShaderNode::new(
        dr.identifier.clone(),
        dr.version,
        dr.name.clone(),
        dr.family.clone(),
        Token::default(), // No context
        dr.source_type.clone(),
        dr.uri.clone(),
        dr.resolved_uri.clone(),
        Vec::new(), // No properties
        metadata,
        dr.source_code.clone(),
    );

    // The node is technically valid as a data structure but represents
    // a failed parse
    Box::new(node)
}

/// A simple passthrough parser that creates nodes directly from discovery results.
///
/// This parser doesn't actually parse any source files - it just creates
/// basic nodes from discovery result metadata. Useful for testing or
/// for nodes that don't require parsing.
pub struct SdrPassthroughParserPlugin {
    discovery_types: SdrTokenVec,
    source_type: Token,
}

impl SdrPassthroughParserPlugin {
    /// Creates a new passthrough parser.
    pub fn new(discovery_types: SdrTokenVec, source_type: Token) -> Self {
        Self {
            discovery_types,
            source_type,
        }
    }
}

impl SdrParserPlugin for SdrPassthroughParserPlugin {
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        // Just create a basic node from the discovery result
        Some(get_invalid_shader_node(discovery_result))
    }

    fn get_discovery_types(&self) -> SdrTokenVec {
        self.discovery_types.clone()
    }

    fn get_source_type(&self) -> Token {
        self.source_type.clone()
    }

    fn get_name(&self) -> &str {
        "SdrPassthroughParserPlugin"
    }
}

#[cfg(test)]
mod tests {
    use super::super::declare::SdrVersion;
    use super::*;

    fn make_test_discovery_result() -> SdrShaderNodeDiscoveryResult {
        SdrShaderNodeDiscoveryResult::minimal(
            Token::new("test_shader"),
            SdrVersion::new(1, 0),
            "test_shader".to_string(),
            Token::new("osl"),
            Token::new("OSL"),
            "/path/to/shader.osl".to_string(),
            "/path/to/shader.osl".to_string(),
        )
    }

    #[test]
    fn test_get_invalid_shader_node() {
        let dr = make_test_discovery_result();
        let node = get_invalid_shader_node(&dr);

        assert_eq!(node.get_identifier().as_str(), "test_shader");
        assert_eq!(node.get_source_type().as_str(), "OSL");
        assert!(node.get_shader_input_names().is_empty());
        assert!(node.get_shader_output_names().is_empty());
    }

    #[test]
    fn test_passthrough_parser() {
        let parser = SdrPassthroughParserPlugin::new(vec![Token::new("osl")], Token::new("OSL"));

        assert_eq!(parser.get_discovery_types().len(), 1);
        assert_eq!(parser.get_source_type().as_str(), "OSL");

        let dr = make_test_discovery_result();
        let node = parser.parse_shader_node(&dr);
        assert!(node.is_some());
    }
}
