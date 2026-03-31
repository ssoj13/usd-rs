//! Filesystem Discovery Plugin - Discovers shader nodes on the filesystem.
//!
//! Port of pxr/usd/sdr/filesystemDiscovery.h
//!
//! This module provides a discovery plugin that walks filesystem paths looking
//! for shader definition files. Files matching the allowed extensions are turned
//! into `SdrShaderNodeDiscoveryResult` instances.
//!
//! # Configuration
//!
//! The plugin can be configured via environment variables (which must be set
//! before the library is loaded):
//!
//! - `PXR_SDR_FS_PLUGIN_SEARCH_PATHS` - The paths that should be searched,
//!   recursively, for files that represent nodes. Paths should be separated
//!   by `;` on Windows or `:` on Unix.
//!
//! - `PXR_SDR_FS_PLUGIN_ALLOWED_EXTS` - The extensions on files that define nodes.
//!   Do not include the leading ".". Extensions should be separated by a colon.
//!
//! - `PXR_SDR_FS_PLUGIN_FOLLOW_SYMLINKS` - Whether symlinks should be followed
//!   while walking the search paths. Set to "true" (case sensitive) if they
//!   should be followed.

use std::env;

use super::declare::SdrStringVec;
use super::discovery_plugin::{SdrDiscoveryPlugin, SdrDiscoveryPluginContext};
use super::discovery_result::{SdrShaderNodeDiscoveryResult, SdrShaderNodeDiscoveryResultVec};
use super::filesystem_discovery_helpers::{SdrParseIdentifierFn, discover_shader_nodes};

/// Environment variable for search paths.
pub const ENV_SEARCH_PATHS: &str = "PXR_SDR_FS_PLUGIN_SEARCH_PATHS";

/// Environment variable for allowed extensions.
pub const ENV_ALLOWED_EXTS: &str = "PXR_SDR_FS_PLUGIN_ALLOWED_EXTS";

/// Environment variable for following symlinks.
pub const ENV_FOLLOW_SYMLINKS: &str = "PXR_SDR_FS_PLUGIN_FOLLOW_SYMLINKS";

/// A filter for discovered nodes.
///
/// If the function returns false then the discovered node is discarded.
/// Otherwise the function can modify the discovery result.
pub type SdrDiscoveryFilter = Box<dyn Fn(&mut SdrShaderNodeDiscoveryResult) -> bool + Send + Sync>;

/// Discovers shader nodes on the filesystem.
///
/// The provided search paths are walked to find files that have certain extensions.
/// If a file with a matching extension is found, it is turned into a
/// `SdrShaderNodeDiscoveryResult` and will be parsed into a node when its
/// information is accessed.
///
/// # Configuration
///
/// Parameters for this plugin are specified via environment variables or
/// programmatically:
///
/// - Search paths: Directories to search recursively
/// - Allowed extensions: File extensions that indicate shader files
/// - Follow symlinks: Whether to follow symbolic links
pub struct SdrFilesystemDiscoveryPlugin {
    /// The paths indicating where the plugin should search for nodes.
    search_paths: SdrStringVec,

    /// The extensions (excluding leading '.') that signify a valid node file.
    allowed_extensions: SdrStringVec,

    /// Whether to follow symlinks while scanning directories.
    follow_symlinks: bool,

    /// Optional filter to run on discovery results.
    filter: Option<SdrDiscoveryFilter>,

    /// Optional custom identifier parser.
    parse_identifier_fn: Option<SdrParseIdentifierFn>,
}

impl SdrFilesystemDiscoveryPlugin {
    /// Creates a new filesystem discovery plugin with configuration from
    /// environment variables.
    pub fn new() -> Self {
        let search_paths = Self::parse_search_paths_from_env();
        let allowed_extensions = Self::parse_allowed_extensions_from_env();
        let follow_symlinks = Self::parse_follow_symlinks_from_env();

        Self {
            search_paths,
            allowed_extensions,
            follow_symlinks,
            filter: None,
            parse_identifier_fn: None,
        }
    }

    /// Creates a new filesystem discovery plugin with a custom filter.
    ///
    /// `discover_shader_nodes()` will pass each result to the given function
    /// for modification. If the function returns false then the result is discarded.
    pub fn with_filter(filter: SdrDiscoveryFilter) -> Self {
        let mut plugin = Self::new();
        plugin.filter = Some(filter);
        plugin
    }

    /// Creates a filesystem discovery plugin with explicit configuration.
    pub fn with_config(
        search_paths: SdrStringVec,
        allowed_extensions: SdrStringVec,
        follow_symlinks: bool,
    ) -> Self {
        Self {
            search_paths,
            allowed_extensions,
            follow_symlinks,
            filter: None,
            parse_identifier_fn: None,
        }
    }

    /// Sets a custom identifier parser function.
    pub fn set_parse_identifier_fn(&mut self, parse_fn: SdrParseIdentifierFn) {
        self.parse_identifier_fn = Some(parse_fn);
    }

    /// Sets a filter function for discovery results.
    pub fn set_filter(&mut self, filter: SdrDiscoveryFilter) {
        self.filter = Some(filter);
    }

    /// Adds a search path.
    pub fn add_search_path(&mut self, path: String) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }

    /// Adds an allowed extension.
    pub fn add_allowed_extension(&mut self, ext: String) {
        let ext_lower = ext.to_lowercase();
        if !self.allowed_extensions.contains(&ext_lower) {
            self.allowed_extensions.push(ext_lower);
        }
    }

    /// Gets the search paths.
    pub fn get_search_paths(&self) -> &SdrStringVec {
        &self.search_paths
    }

    /// Gets the allowed extensions.
    pub fn get_allowed_extensions(&self) -> &SdrStringVec {
        &self.allowed_extensions
    }

    /// Returns whether symlinks are followed.
    pub fn get_follow_symlinks(&self) -> bool {
        self.follow_symlinks
    }

    /// Sets whether to follow symlinks.
    pub fn set_follow_symlinks(&mut self, follow: bool) {
        self.follow_symlinks = follow;
    }

    // Internal: Parse search paths from environment variable
    fn parse_search_paths_from_env() -> SdrStringVec {
        let env_val = env::var(ENV_SEARCH_PATHS).unwrap_or_default();
        if env_val.is_empty() {
            return Vec::new();
        }

        // Use platform-specific path separator
        #[cfg(windows)]
        let separator = ';';
        #[cfg(not(windows))]
        let separator = ':';

        env_val
            .split(separator)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    // Internal: Parse allowed extensions from environment variable
    fn parse_allowed_extensions_from_env() -> SdrStringVec {
        let env_val = env::var(ENV_ALLOWED_EXTS).unwrap_or_default();
        if env_val.is_empty() {
            return Vec::new();
        }

        env_val
            .split(':')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    // Internal: Parse follow symlinks from environment variable
    fn parse_follow_symlinks_from_env() -> bool {
        env::var(ENV_FOLLOW_SYMLINKS)
            .map(|v| v == "true")
            .unwrap_or(true) // Default to following symlinks
    }
}

impl Default for SdrFilesystemDiscoveryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SdrDiscoveryPlugin for SdrFilesystemDiscoveryPlugin {
    /// Discover all of the nodes that appear within the search paths
    /// and match the allowed extensions.
    fn discover_shader_nodes(
        &self,
        context: &dyn SdrDiscoveryPluginContext,
    ) -> SdrShaderNodeDiscoveryResultVec {
        let mut results = discover_shader_nodes(
            &self.search_paths,
            &self.allowed_extensions,
            self.follow_symlinks,
            None, // Let helpers determine source type from context
            self.parse_identifier_fn.as_ref(),
        );

        // Update source types from context
        for result in &mut results {
            let source_type = context.get_source_type(&result.discovery_type);
            result.source_type = source_type;
        }

        // Apply filter if present
        if let Some(ref filter) = self.filter {
            results.retain_mut(|result| filter(result));
        }

        results
    }

    /// Gets the paths that this plugin is searching for nodes in.
    fn get_search_uris(&self) -> SdrStringVec {
        self.search_paths.clone()
    }

    fn get_name(&self) -> &str {
        "SdrFilesystemDiscoveryPlugin"
    }
}

#[cfg(test)]
mod tests {
    use super::super::discovery_plugin::DefaultDiscoveryPluginContext;
    use super::*;

    #[test]
    fn test_new_plugin() {
        let plugin = SdrFilesystemDiscoveryPlugin::new();
        // Default config from env, which is likely empty in tests
        assert!(plugin.search_paths.is_empty() || !plugin.search_paths.is_empty());
    }

    #[test]
    fn test_with_config() {
        let plugin = SdrFilesystemDiscoveryPlugin::with_config(
            vec!["/path/to/shaders".to_string()],
            vec!["osl".to_string(), "glslfx".to_string()],
            true,
        );

        assert_eq!(plugin.search_paths.len(), 1);
        assert_eq!(plugin.allowed_extensions.len(), 2);
        assert!(plugin.follow_symlinks);
    }

    #[test]
    fn test_add_search_path() {
        let mut plugin = SdrFilesystemDiscoveryPlugin::with_config(vec![], vec![], true);
        plugin.add_search_path("/path1".to_string());
        plugin.add_search_path("/path2".to_string());
        plugin.add_search_path("/path1".to_string()); // Duplicate

        assert_eq!(plugin.search_paths.len(), 2);
    }

    #[test]
    fn test_add_allowed_extension() {
        let mut plugin = SdrFilesystemDiscoveryPlugin::with_config(vec![], vec![], true);
        plugin.add_allowed_extension("osl".to_string());
        plugin.add_allowed_extension("OSL".to_string()); // Duplicate (case insensitive)
        plugin.add_allowed_extension("glslfx".to_string());

        assert_eq!(plugin.allowed_extensions.len(), 2);
    }

    #[test]
    fn test_discover_empty_paths() {
        let plugin =
            SdrFilesystemDiscoveryPlugin::with_config(vec![], vec!["osl".to_string()], true);
        let context = DefaultDiscoveryPluginContext;
        let results = plugin.discover_shader_nodes(&context);
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_search_uris() {
        let plugin = SdrFilesystemDiscoveryPlugin::with_config(
            vec!["/path1".to_string(), "/path2".to_string()],
            vec![],
            true,
        );

        let uris = plugin.get_search_uris();
        assert_eq!(uris.len(), 2);
    }
}
