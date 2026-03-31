
//! RAII handle for render delegate created by plugin.
//!
//! Corresponds to pxr/imaging/hd/pluginRenderDelegateUniqueHandle.h.
//! Keeps the plugin alive until the delegate is destroyed.

use crate::render::HdRenderDelegateSharedPtr;
use crate::renderer_plugin_handle::HdRendererPluginHandle;
use usd_tf::Token;

/// Handle owning a render delegate created by a plugin.
///
/// The handle keeps the plugin alive until the delegate is destroyed.
/// Corresponds to C++ `HdPluginRenderDelegateUniqueHandle`.
pub struct HdPluginRenderDelegateUniqueHandle {
    _plugin: Option<HdRendererPluginHandle>,
    delegate: Option<HdRenderDelegateSharedPtr>,
}

impl HdPluginRenderDelegateUniqueHandle {
    /// Create from plugin and delegate (internal use by HdRendererPlugin).
    #[allow(dead_code)] // Called by HdRendererPlugin when plugin system is wired
    pub(crate) fn new(plugin: HdRendererPluginHandle, delegate: HdRenderDelegateSharedPtr) -> Self {
        Self {
            _plugin: Some(plugin),
            delegate: Some(delegate),
        }
    }

    /// Create empty (null) handle.
    pub fn empty() -> Self {
        Self {
            _plugin: None,
            delegate: None,
        }
    }

    /// Get reference to the delegate, if valid.
    pub fn get(&self) -> Option<&HdRenderDelegateSharedPtr> {
        self.delegate.as_ref()
    }

    /// Get plugin ID used to create this delegate.
    pub fn get_plugin_id(&self) -> Token {
        self._plugin
            .as_ref()
            .map(|p| p.get_plugin_id())
            .unwrap_or_else(|| Token::new(""))
    }

    /// Check if handle is valid.
    pub fn is_valid(&self) -> bool {
        self.delegate.is_some()
    }
}

impl Default for HdPluginRenderDelegateUniqueHandle {
    fn default() -> Self {
        Self::empty()
    }
}
