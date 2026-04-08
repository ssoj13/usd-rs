//! HdRenderDelegateInfo - Query result from render delegate.
//!
//! Corresponds to pxr/imaging/hd/renderDelegateInfo.h.

use usd_tf::Token;

/// Token vector type.
pub type TfTokenVector = Vec<Token>;

/// Info queried from a render delegate.
///
/// Corresponds to C++ `HdRenderDelegateInfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HdRenderDelegateInfo {
    /// Material binding purpose.
    pub material_binding_purpose: Token,

    /// Material render contexts supported.
    pub material_render_contexts: TfTokenVector,

    /// Render settings namespaces.
    pub render_settings_namespaces: TfTokenVector,

    /// Whether primvar filtering is needed.
    pub is_primvar_filtering_needed: bool,

    /// Shader source types.
    pub shader_source_types: TfTokenVector,

    /// Whether coordinate systems are supported.
    pub is_coord_sys_supported: bool,
}

impl HdRenderDelegateInfo {
    /// Create new with defaults.
    pub fn new() -> Self {
        Self::default()
    }
}
