
//! HdStRenderPassShader - Shader configuration per render pass.
//!
//! Provides shader mixins for render pass functionality including:
//! - AOV (Arbitrary Output Variable) readback
//! - Clip plane evaluation
//! - Selection highlighting
//! - Custom buffer bindings

use crate::binding::BindingRequest;
use crate::shader_code::{HdStShaderCode, NamedTextureHandle, ShaderParameter, ShaderStage};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_tf::Token;

/// Shared pointer type.
pub type HdStRenderPassShaderSharedPtr = Arc<HdStRenderPassShader>;

/// Render pass shader configuration.
///
/// Manages per-pass shader code that handles:
/// - AOV output routing (color, depth, id, normal, etc.)
/// - Clip plane evaluation in fragment shader
/// - Selection highlight overlay
/// - Custom buffer bindings for render pass data
#[derive(Debug)]
pub struct HdStRenderPassShader {
    /// Unique ID
    id: u64,

    /// Custom buffer bindings (lexicographically ordered for stability)
    custom_buffers: BTreeMap<Token, BindingRequest>,

    /// Named texture handles for AOV readback
    named_texture_handles: Vec<NamedTextureHandle>,

    /// WGSL source snippets per shader stage
    wgsl_sources: BTreeMap<ShaderStage, String>,

    /// Whether AOV readback is configured
    has_aov_readback: bool,

    /// Clip plane count (0 = disabled)
    clip_plane_count: u32,

    /// Selection highlight enabled
    selection_highlight: bool,
}

impl HdStRenderPassShader {
    /// Create a default render pass shader.
    pub fn new() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            custom_buffers: BTreeMap::new(),
            named_texture_handles: Vec::new(),
            wgsl_sources: BTreeMap::new(),
            has_aov_readback: false,
            clip_plane_count: 0,
            selection_highlight: false,
        }
    }

    /// Create with initial WGSL source for a given shader stage.
    pub fn with_source(stage: ShaderStage, source: String) -> Self {
        let mut shader = Self::new();
        shader.wgsl_sources.insert(stage, source);
        shader
    }

    /// Add a custom buffer binding for the render pass.
    pub fn add_buffer_binding(&mut self, req: BindingRequest) {
        self.custom_buffers.insert(req.name.clone(), req);
    }

    /// Remove a custom buffer binding by name.
    pub fn remove_buffer_binding(&mut self, name: &Token) {
        self.custom_buffers.remove(name);
    }

    /// Clear all custom buffer bindings.
    pub fn clear_buffer_bindings(&mut self) {
        self.custom_buffers.clear();
    }

    /// Set the number of clip planes.
    pub fn set_clip_plane_count(&mut self, count: u32) {
        self.clip_plane_count = count;
    }

    /// Get clip plane count.
    pub fn get_clip_plane_count(&self) -> u32 {
        self.clip_plane_count
    }

    /// Enable/disable selection highlight in fragment shader.
    pub fn set_selection_highlight(&mut self, enabled: bool) {
        self.selection_highlight = enabled;
    }

    /// Whether selection highlight is enabled.
    pub fn has_selection_highlight(&self) -> bool {
        self.selection_highlight
    }

    /// Get the custom buffer bindings.
    pub fn get_custom_bindings(&self) -> Vec<BindingRequest> {
        self.custom_buffers.values().cloned().collect()
    }

    /// Get named texture handles.
    pub fn get_named_texture_handles(&self) -> &[NamedTextureHandle] {
        &self.named_texture_handles
    }

    /// Check if AOV readback is configured.
    pub fn has_aov_readback(&self) -> bool {
        self.has_aov_readback
    }
}

impl Default for HdStRenderPassShader {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStShaderCode for HdStRenderPassShader {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn get_source(&self, stage: ShaderStage) -> String {
        if let Some(src) = self.wgsl_sources.get(&stage) {
            return src.clone();
        }

        // Generate stage-specific code on the fly
        let mut src = String::new();

        if stage == ShaderStage::Fragment {
            use std::fmt::Write;

            // Clip plane evaluation
            if self.clip_plane_count > 0 {
                writeln!(src, "// Clip plane evaluation ({} planes)", self.clip_plane_count).unwrap();
                writeln!(src, "fn eval_clip_planes(world_pos: vec3<f32>) -> bool {{").unwrap();
                for i in 0..self.clip_plane_count {
                    writeln!(src, "    if (dot(vec4<f32>(world_pos, 1.0), clip_plane_{}) < 0.0) {{ return false; }}", i).unwrap();
                }
                writeln!(src, "    return true;").unwrap();
                writeln!(src, "}}").unwrap();
            }

            // Selection highlight
            if self.selection_highlight {
                writeln!(src, "// Selection highlight overlay").unwrap();
                writeln!(src, "fn apply_selection_highlight(color: vec4<f32>, selected: bool) -> vec4<f32> {{").unwrap();
                writeln!(src, "    if (selected) {{").unwrap();
                writeln!(src, "        return mix(color, vec4<f32>(1.0, 1.0, 0.0, 1.0), 0.3);").unwrap();
                writeln!(src, "    }}").unwrap();
                writeln!(src, "    return color;").unwrap();
                writeln!(src, "}}").unwrap();
            }
        }

        src
    }

    fn get_params(&self) -> Vec<ShaderParameter> {
        Vec::new()
    }

    fn get_textures(&self) -> Vec<NamedTextureHandle> {
        self.named_texture_handles.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::BindingType;

    #[test]
    fn test_render_pass_shader_default() {
        let shader = HdStRenderPassShader::new();
        assert!(!shader.has_aov_readback());
        assert!(!shader.has_selection_highlight());
        assert_eq!(shader.get_clip_plane_count(), 0);
    }

    #[test]
    fn test_custom_buffer_bindings() {
        let mut shader = HdStRenderPassShader::new();

        let req = BindingRequest {
            name: Token::new("selectionBuffer"),
            binding_type: BindingType::Uniform,
            data_type: Token::new("vec4f"),
        };

        shader.add_buffer_binding(req);
        assert_eq!(shader.get_custom_bindings().len(), 1);

        shader.remove_buffer_binding(&Token::new("selectionBuffer"));
        assert!(shader.get_custom_bindings().is_empty());
    }

    #[test]
    fn test_clip_planes() {
        let mut shader = HdStRenderPassShader::new();
        shader.set_clip_plane_count(3);

        let fs_source = shader.get_source(ShaderStage::Fragment);
        assert!(fs_source.contains("clip_plane_"));
        assert!(fs_source.contains("3 planes"));
    }

    #[test]
    fn test_selection_highlight() {
        let mut shader = HdStRenderPassShader::new();
        shader.set_selection_highlight(true);

        let fs_source = shader.get_source(ShaderStage::Fragment);
        assert!(fs_source.contains("apply_selection_highlight"));
    }
}
