//! Renderer plugin interface.
//!
//! Corresponds to pxr/imaging/hd/rendererPlugin.h.
//! Defines the plugin interface for Hydra renderers.

use super::plugin_render_delegate_unique_handle::HdPluginRenderDelegateUniqueHandle;
use super::renderer_create_args::HdRendererCreateArgs;
use super::renderer_plugin_handle::HdRendererPluginTrait;
use crate::data_source::HdContainerDataSourceHandle;
use crate::render::{HdRenderDelegateSharedPtr, HdRenderSettingsMap};
use std::sync::Arc;

/// Renderer plugin interface for Hydra.
///
/// Corresponds to C++ `HdRendererPlugin`.
/// A plugin is instantiated once per library and used to create render delegates.
pub trait HdRendererPlugin: HdRendererPluginTrait + Send + Sync {
    /// Returns true if this renderer is supported (e.g., GPU available).
    fn is_supported(&self, args: &HdRendererCreateArgs) -> bool;

    /// Returns optional reason why the renderer is not supported.
    fn is_supported_reason(&self, _args: &HdRendererCreateArgs) -> Option<String> {
        None
    }

    /// Get scene index input args for configuring scene indices.
    fn get_scene_index_input_args(&self) -> Option<HdContainerDataSourceHandle> {
        None
    }

    /// Create a render delegate. Keeps this plugin alive until delegate is dropped.
    fn create_delegate(&self, settings: &HdRenderSettingsMap)
    -> HdPluginRenderDelegateUniqueHandle;

    /// Create render delegate (internal factory). Override in implementors.
    fn create_render_delegate(&self) -> Option<HdRenderDelegateSharedPtr> {
        None
    }

    /// Create render delegate with initial settings.
    fn create_render_delegate_with_settings(
        &self,
        _settings: &HdRenderSettingsMap,
    ) -> Option<HdRenderDelegateSharedPtr> {
        self.create_render_delegate()
    }
}

/// Type alias for plugin handle.
pub type HdRendererPluginHandleType = Arc<dyn HdRendererPlugin>;
