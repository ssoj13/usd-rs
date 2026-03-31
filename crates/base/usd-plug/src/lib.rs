//! Plugin Registry (plug) - Plugin discovery, registration and metadata.
//!
//! Port of pxr/base/plug/
//!
//! Provides the central plugin system for OpenUSD. Discovers plugins via
//! `plugInfo.json` files, stores their metadata, and provides lookup
//! by name, path, or declared type.
//!
//! # Architecture
//!
//! - `PlugRegistry` (singleton) - central registry for all plugins
//! - `PlugPlugin` - represents a single registered plugin with metadata
//! - `plugInfo.json` parser - recursive discovery with glob/wildcard support
//! - `PlugNotice` - notification after new plugins registered
//!
//! # Plugin Types
//!
//! - `library` - native shared library (cdylib)
//! - `resource` - metadata-only (no code loading)
//!
//! # Example
//!
//! ```ignore
//! use usd_plug::PlugRegistry;
//!
//! // Register plugins from a directory
//! let registry = PlugRegistry::get_instance();
//! let new_plugins = registry.register_plugins("/path/to/plugins");
//!
//! // Query all plugins
//! for plugin in registry.get_all_plugins() {
//!     println!("{}: {:?}", plugin.get_name(), plugin.get_type());
//! }
//!
//! // Lookup by name
//! if let Some(plugin) = registry.get_plugin_with_name("myPlugin") {
//!     let metadata = plugin.get_metadata();
//! }
//! ```

pub mod info;
pub mod init_config;
pub mod interface_factory;
pub mod notice;
pub mod plugin;
pub mod registry;
pub mod static_interface;

pub use info::RegistrationMetadata;
pub use notice::on_did_register_plugins;
pub use plugin::{PlugPlugin, PluginType, find_plugin_resource};
pub use registry::PlugRegistry;
