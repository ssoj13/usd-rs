//! Discovery Plugin - Interface for shader node discovery plugins.
//!
//! Port of pxr/usd/sdr/discoveryPlugin.h
//!
//! This module defines the interface for discovery plugins that find shader nodes.
//! Discovery plugins search various sources (filesystem, cloud, database) and report
//! what nodes they found via `SdrShaderNodeDiscoveryResult` instances.
//!
//! # Architecture
//!
//! Discovery plugins simply report back to the registry what nodes they found.
//! The registry doesn't know much about the innards of the nodes yet, just that
//! the nodes exist. Understanding the nodes is the responsibility of parser plugins.

use super::declare::SdrStringVec;
use super::discovery_result::SdrShaderNodeDiscoveryResultVec;
use usd_tf::Token;

/// A context for discovery.
///
/// Discovery plugins can use this to get a limited set of non-local information
/// without direct coupling between plugins.
pub trait SdrDiscoveryPluginContext: Send + Sync {
    /// Returns the source type for the given discovery type.
    ///
    /// This allows mapping from discovery types (e.g., file extensions like "osl")
    /// to source types (e.g., "OSL").
    fn get_source_type(&self, discovery_type: &Token) -> Token;
}

/// Default implementation of discovery plugin context.
#[derive(Debug, Default)]
pub struct DefaultDiscoveryPluginContext;

impl SdrDiscoveryPluginContext for DefaultDiscoveryPluginContext {
    fn get_source_type(&self, discovery_type: &Token) -> Token {
        // Default: source type is same as discovery type
        discovery_type.clone()
    }
}

/// Interface for discovery plugins for finding shader nodes.
///
/// Discovery plugins, like the name implies, find nodes. Where the plugin
/// searches is up to the plugin that implements this interface. Examples
/// of discovery plugins could include plugins that look for nodes on the
/// filesystem, another that finds nodes in a cloud service, and another that
/// searches a local database.
///
/// Multiple discovery plugins that search the filesystem in specific locations/ways
/// could also be created. All discovery plugins are executed as soon as the
/// registry is instantiated.
///
/// # Implementation Notes
///
/// These plugins simply report back to the registry what nodes they found in
/// a generic way. The registry doesn't know much about the innards of the
/// nodes yet, just that the nodes exist. Understanding the nodes is the
/// responsibility of another set of plugins defined by `SdrParserPlugin`.
///
/// Discovery plugins report back to the registry via `SdrShaderNodeDiscoveryResult`s.
/// These are small, lightweight classes that contain the information for a
/// single node that was found during discovery.
pub trait SdrDiscoveryPlugin: Send + Sync {
    /// Finds and returns all nodes that the implementing plugin should be aware of.
    ///
    /// The context provides access to non-local information that may be useful
    /// during discovery (e.g., mapping discovery types to source types).
    fn discover_shader_nodes(
        &self,
        context: &dyn SdrDiscoveryPluginContext,
    ) -> SdrShaderNodeDiscoveryResultVec;

    /// Gets the URIs that this plugin is searching for nodes in.
    ///
    /// Returns the search paths/URIs that this discovery plugin will traverse
    /// when looking for shader nodes.
    fn get_search_uris(&self) -> SdrStringVec;

    /// Returns the name of this discovery plugin for identification purposes.
    fn get_name(&self) -> &str {
        "SdrDiscoveryPlugin"
    }
}

/// A boxed discovery plugin for type-erased storage.
pub type SdrDiscoveryPluginRef = Box<dyn SdrDiscoveryPlugin>;

/// A vector of discovery plugin references.
pub type SdrDiscoveryPluginRefVec = Vec<SdrDiscoveryPluginRef>;

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDiscoveryPlugin {
        search_uris: SdrStringVec,
    }

    impl SdrDiscoveryPlugin for MockDiscoveryPlugin {
        fn discover_shader_nodes(
            &self,
            _context: &dyn SdrDiscoveryPluginContext,
        ) -> SdrShaderNodeDiscoveryResultVec {
            vec![]
        }

        fn get_search_uris(&self) -> SdrStringVec {
            self.search_uris.clone()
        }

        fn get_name(&self) -> &str {
            "MockDiscoveryPlugin"
        }
    }

    #[test]
    fn test_default_context() {
        let context = DefaultDiscoveryPluginContext;
        let discovery_type = Token::new("osl");
        let source_type = context.get_source_type(&discovery_type);
        assert_eq!(source_type.as_str(), "osl");
    }

    #[test]
    fn test_mock_plugin() {
        let plugin = MockDiscoveryPlugin {
            search_uris: vec!["/path/to/shaders".to_string()],
        };

        assert_eq!(plugin.get_name(), "MockDiscoveryPlugin");
        assert_eq!(plugin.get_search_uris().len(), 1);
    }
}
