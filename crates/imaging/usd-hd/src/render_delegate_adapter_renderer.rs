
//! HdRenderDelegateAdapterRenderer - Renderer that populates legacy render delegate.
//!
//! Corresponds to pxr/imaging/hd/renderDelegateAdapterRenderer.h.

use super::plugin_render_delegate_unique_handle::HdPluginRenderDelegateUniqueHandle;
use super::renderer::{HdLegacyRenderControlInterface, HdRenderer};
use crate::data_source::HdContainerDataSourceHandle;
use std::sync::Arc;

/// Renderer that populates HdRenderDelegate from a scene index (back-end emulation).
///
/// Corresponds to C++ `HdRenderDelegateAdapterRenderer`.
pub struct HdRenderDelegateAdapterRenderer {
    _render_delegate: HdPluginRenderDelegateUniqueHandle,
    _legacy_control: Option<Box<dyn HdLegacyRenderControlInterface>>,
}

impl HdRenderDelegateAdapterRenderer {
    /// Create from plugin delegate, terminal scene index, and create args.
    pub fn new(
        render_delegate: HdPluginRenderDelegateUniqueHandle,
        _terminal_scene_index: Arc<dyn std::any::Any>,
        _renderer_create_args: HdContainerDataSourceHandle,
    ) -> Self {
        Self {
            _render_delegate: render_delegate,
            _legacy_control: None,
        }
    }
}

impl HdRenderer for HdRenderDelegateAdapterRenderer {
    fn get_legacy_render_control(&self) -> Option<&dyn HdLegacyRenderControlInterface> {
        self._legacy_control.as_deref()
    }
}
