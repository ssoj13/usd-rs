//! SDR Shader Node Discovery Result - Raw discovery data for shader nodes.
//!
//! Port of pxr/usd/sdr/shaderNodeDiscoveryResult.h
//!
//! This module provides SdrShaderNodeDiscoveryResult which represents the raw
//! data of a node determined via a discovery plugin, before parsing.
//! Discovery results contain basic metadata that doesn't require parsing
//! the shader's contents.
//!
//! Used by: SdrRegistry, SdrDiscoveryPlugin
//! Uses: SdrIdentifier, SdrVersion, SdrTokenMap

use super::declare::{SdrIdentifier, SdrTokenMap, SdrVersion};
use usd_tf::Token;

/// Represents the raw data of a node, and some other bits of metadata, that
/// were determined via a discovery plugin.
///
/// This struct contains all information that can be gathered about a shader
/// node without actually parsing its contents. It's produced by discovery
/// plugins and consumed by the registry when creating full shader nodes.
///
/// # Fields
///
/// - `identifier`: Unique identifier for the node (e.g., "mix_float_2_1")
/// - `version`: Version of the node (e.g., 2.1)
/// - `name`: Version-independent name (e.g., "mix_float")
/// - `family`: Optional grouping (e.g., "mix")
/// - `discovery_type`: Type hint for parser selection (e.g., file extension)
/// - `source_type`: Source origin (e.g., "osl", "glslfx")
/// - `uri`: Original resource location
/// - `resolved_uri`: Fully resolved URI for local access
/// - `source_code`: Optional inline source code
/// - `metadata`: Additional discovery-time metadata
/// - `blind_data`: Parser-specific data
/// - `sub_identifier`: For multi-definition assets
#[derive(Debug, Clone, Default)]
pub struct SdrShaderNodeDiscoveryResult {
    /// The node's identifier.
    ///
    /// How the node is identified. In many cases this will be the
    /// name of the file or resource that this node originated from.
    /// E.g. "mix_float_2_1". The identifier must be unique for a
    /// given sourceType.
    pub identifier: SdrIdentifier,

    /// The node's version.
    ///
    /// This may or may not be embedded in the identifier, it's up to
    /// implementations. E.g a node with identifier "mix_float_2_1"
    /// might have version 2.1.
    pub version: SdrVersion,

    /// The node's name.
    ///
    /// A version independent identifier for the node type. This will
    /// often embed type parameterization but should not embed the
    /// version. E.g a node with identifier "mix_float_2_1" might have
    /// name "mix_float".
    pub name: String,

    /// The node's family.
    ///
    /// A node's family is an optional piece of metadata that specifies a
    /// generic grouping of nodes. E.g a node with identifier
    /// "mix_float_2_1" might have family "mix".
    pub family: Token,

    /// The node's discovery type.
    ///
    /// The type could be the file extension, or some other type of metadata
    /// that can signify a type prior to parsing. See the documentation for
    /// SdrParserPlugin and SdrParserPlugin::DiscoveryTypes for more
    /// information on how this value is used.
    pub discovery_type: Token,

    /// The node's source type.
    ///
    /// This type is unique to the parsing plugin (SdrParserPlugin::SourceType),
    /// and determines the source of the node. See SdrShaderNode::GetSourceType()
    /// for more information.
    pub source_type: Token,

    /// The node's origin.
    ///
    /// This may be a filesystem path, a URL pointing to a resource in the
    /// cloud, or some other type of resource identifier.
    pub uri: String,

    /// The node's fully-resolved URI.
    ///
    /// For example, this might be an absolute path when the original URI was
    /// a relative path. In most cases, this is the path that Ar's Resolve()
    /// returns. In any case, this path should be locally accessible.
    pub resolved_uri: String,

    /// The node's entire source code.
    ///
    /// The source code is parsed (if non-empty) by parser plugins when the
    /// resolvedUri value is empty.
    pub source_code: String,

    /// The node's metadata collected during the discovery process.
    ///
    /// Additional metadata may be present in the node's source, in the asset
    /// pointed to by resolvedUri or in sourceCode (if resolvedUri is empty).
    /// In general, parsers should override this data with metadata from the
    /// shader source.
    pub metadata: SdrTokenMap,

    /// An optional detail for the parser plugin.
    ///
    /// The parser plugin defines the meaning of this data so the discovery
    /// plugin must be written to match.
    pub blind_data: String,

    /// The subIdentifier is associated with a particular asset and refers to a
    /// specific definition within the asset.
    ///
    /// The asset is the one referred to by SdrRegistry::GetNodeFromAsset().
    /// The subIdentifier is not needed for all cases where the node definition
    /// is not associated with an asset. Even if the node definition is
    /// associated with an asset, the subIdentifier is only needed if the asset
    /// specifies multiple definitions rather than a single definition.
    pub sub_identifier: Token,
}

impl SdrShaderNodeDiscoveryResult {
    /// Creates a new discovery result with all fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: SdrIdentifier,
        version: SdrVersion,
        name: String,
        family: Token,
        discovery_type: Token,
        source_type: Token,
        uri: String,
        resolved_uri: String,
        source_code: String,
        metadata: SdrTokenMap,
        blind_data: String,
        sub_identifier: Token,
    ) -> Self {
        Self {
            identifier,
            version,
            name,
            family,
            discovery_type,
            source_type,
            uri,
            resolved_uri,
            source_code,
            metadata,
            blind_data,
            sub_identifier,
        }
    }

    /// Creates a minimal discovery result with required fields only.
    pub fn minimal(
        identifier: SdrIdentifier,
        version: SdrVersion,
        name: String,
        discovery_type: Token,
        source_type: Token,
        uri: String,
        resolved_uri: String,
    ) -> Self {
        Self {
            identifier,
            version,
            name,
            family: Token::default(),
            discovery_type,
            source_type,
            uri,
            resolved_uri,
            source_code: String::new(),
            metadata: SdrTokenMap::new(),
            blind_data: String::new(),
            sub_identifier: Token::default(),
        }
    }

    /// Creates a discovery result from source code instead of a file.
    pub fn from_source_code(
        identifier: SdrIdentifier,
        version: SdrVersion,
        name: String,
        discovery_type: Token,
        source_type: Token,
        source_code: String,
    ) -> Self {
        Self {
            identifier,
            version,
            name,
            family: Token::default(),
            discovery_type,
            source_type,
            uri: String::new(),
            resolved_uri: String::new(),
            source_code,
            metadata: SdrTokenMap::new(),
            blind_data: String::new(),
            sub_identifier: Token::default(),
        }
    }

    /// Returns true if this discovery result has source code.
    pub fn has_source_code(&self) -> bool {
        !self.source_code.is_empty()
    }

    /// Returns true if this discovery result has a resolved URI.
    pub fn has_resolved_uri(&self) -> bool {
        !self.resolved_uri.is_empty()
    }

    /// Returns true if this discovery result can be parsed.
    ///
    /// A discovery result can be parsed if it has either a resolved URI or source code.
    pub fn is_parseable(&self) -> bool {
        self.has_resolved_uri() || self.has_source_code()
    }
}

/// Vector of discovery results.
pub type SdrShaderNodeDiscoveryResultVec = Vec<SdrShaderNodeDiscoveryResult>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_discovery_result() {
        let result = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("test_shader"),
            SdrVersion::new(1, 0),
            "test_shader".to_string(),
            Token::new("osl"),
            Token::new("OSL"),
            "/path/to/shader.osl".to_string(),
            "/absolute/path/to/shader.osl".to_string(),
        );

        assert_eq!(result.identifier.as_str(), "test_shader");
        assert_eq!(result.version.major(), 1);
        assert_eq!(result.name, "test_shader");
        assert!(result.has_resolved_uri());
        assert!(!result.has_source_code());
        assert!(result.is_parseable());
    }

    #[test]
    fn test_source_code_discovery_result() {
        let result = SdrShaderNodeDiscoveryResult::from_source_code(
            Token::new("inline_shader"),
            SdrVersion::new(1, 0),
            "inline_shader".to_string(),
            Token::new("osl"),
            Token::new("OSL"),
            "shader test() {}".to_string(),
        );

        assert!(result.has_source_code());
        assert!(!result.has_resolved_uri());
        assert!(result.is_parseable());
    }

    #[test]
    fn test_default_not_parseable() {
        let result = SdrShaderNodeDiscoveryResult::default();
        assert!(!result.is_parseable());
    }
}
