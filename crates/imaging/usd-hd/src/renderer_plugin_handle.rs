//! Renderer plugin handle - RAII handle keeping plugin alive.
//!
//! Corresponds to pxr/imaging/hd/rendererPluginHandle.h.

use std::sync::Arc;
use usd_tf::Token;

/// Opaque trait for renderer plugin (avoids circular dep with full HdRendererPlugin).
pub trait HdRendererPluginTrait: Send + Sync {
    /// Get plugin ID from registry.
    fn get_plugin_id(&self) -> Token;
}

/// Shared handle to a renderer plugin.
///
/// Corresponds to C++ `HdRendererPluginHandle`.
pub type HdRendererPluginHandle = Arc<dyn HdRendererPluginTrait>;
