
//! RAII handle for HdRenderer created by plugin.
//!
//! Corresponds to pxr/imaging/hd/pluginRendererUniqueHandle.h.

use super::renderer::HdRenderer;
use super::renderer_plugin_handle::HdRendererPluginHandle;
use usd_tf::Token;

/// Handle owning an HdRenderer created by a plugin.
///
/// Keeps the plugin alive until the renderer is destroyed.
/// Corresponds to C++ `HdPluginRendererUniqueHandle`.
pub struct HdPluginRendererUniqueHandle {
    _plugin: Option<HdRendererPluginHandle>,
    renderer: Option<Box<dyn HdRenderer>>,
}

impl HdPluginRendererUniqueHandle {
    /// Create from plugin and renderer (internal use by HdRendererPlugin).
    #[allow(dead_code)] // Called by HdRendererPlugin when plugin system is wired
    pub(crate) fn new(plugin: HdRendererPluginHandle, renderer: Box<dyn HdRenderer>) -> Self {
        Self {
            _plugin: Some(plugin),
            renderer: Some(renderer),
        }
    }

    /// Create empty (null) handle.
    pub fn empty() -> Self {
        Self {
            _plugin: None,
            renderer: None,
        }
    }

    /// Get reference to renderer.
    pub fn get(&self) -> Option<&dyn HdRenderer> {
        self.renderer.as_deref()
    }

    /// Get plugin ID.
    pub fn get_plugin_id(&self) -> Token {
        self._plugin
            .as_ref()
            .map(|p| p.get_plugin_id())
            .unwrap_or_else(|| Token::new(""))
    }

    /// Check if valid.
    pub fn is_valid(&self) -> bool {
        self.renderer.is_some()
    }
}

impl Default for HdPluginRendererUniqueHandle {
    fn default() -> Self {
        Self::empty()
    }
}
