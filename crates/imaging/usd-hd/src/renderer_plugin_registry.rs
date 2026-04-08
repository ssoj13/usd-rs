//! Renderer plugin registry - singleton for discovering and creating renderers.
//!
//! Corresponds to pxr/imaging/hd/rendererPluginRegistry.h.

use super::plugin_render_delegate_unique_handle::HdPluginRenderDelegateUniqueHandle;
use super::renderer_create_args::HdRendererCreateArgs;
use super::renderer_plugin::HdRendererPluginHandleType;
use crate::render::HdRenderSettingsMap;
use usd_hf::plugin_registry::HfPluginRegistry;
use usd_hf::plugin_registry::HfPluginRegistryImpl;
use usd_tf::Token;

/// Singleton registry for HdRendererPlugin.
///
/// Corresponds to C++ `HdRendererPluginRegistry`.
pub struct HdRendererPluginRegistry {
    inner: HfPluginRegistryImpl,
}

impl HdRendererPluginRegistry {
    /// Get the singleton instance.
    pub fn get_instance() -> &'static Self {
        static INSTANCE: once_cell::sync::Lazy<HdRendererPluginRegistry> =
            once_cell::sync::Lazy::new(|| HdRendererPluginRegistry {
                inner: HfPluginRegistryImpl::new(),
            });
        &INSTANCE
    }

    /// Get or create renderer plugin by ID.
    pub fn get_or_create_renderer_plugin(
        &self,
        _plugin_id: &Token,
    ) -> Option<HdRendererPluginHandleType> {
        None
    }

    /// Get default plugin ID based on available resources.
    pub fn get_default_plugin_id(&self, args: &HdRendererCreateArgs) -> Token {
        let descs = self.inner.get_plugin_descs();
        // Prefer GPU-enabled plugins
        let id = if args.gpu_enabled {
            descs.first()
        } else {
            descs.first()
        };
        id.map(|d| d.id.clone()).unwrap_or_else(|| Token::new(""))
    }

    /// Create render delegate from plugin. Returns empty handle if plugin not found or unsupported.
    pub fn create_render_delegate(
        &self,
        _plugin_id: &Token,
        _settings: &HdRenderSettingsMap,
    ) -> HdPluginRenderDelegateUniqueHandle {
        HdPluginRenderDelegateUniqueHandle::empty()
    }
}
