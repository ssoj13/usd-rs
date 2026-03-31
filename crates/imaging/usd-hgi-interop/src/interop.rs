//! HgiInterop - Main interoperability interface.
//!
//! Provides texture transfer between different HGI backends and presentation
//! to a wgpu surface.

use std::sync::Arc;

use super::wgpu_interop::HgiInteropWgpu;

/// HgiInterop composites HGI render results onto a wgpu surface.
///
/// Unlike the C++ implementation which needs separate GL/Vulkan/Metal interop
/// classes, this uses wgpu as the single presentation backend.
///
/// # Usage
///
/// ```rust,ignore
/// let interop = HgiInterop::new(device.clone(), queue.clone());
///
/// // Each frame, after rendering to an HGI texture:
/// interop.composite(&color_view, None, &surface_view, None, surface_format, viewport);
/// ```
pub struct HgiInterop {
    wgpu: HgiInteropWgpu,
}

impl HgiInterop {
    /// Create a new HgiInterop backed by the given wgpu device and queue.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            wgpu: HgiInteropWgpu::new(device, queue),
        }
    }

    /// Composite provided textures over the destination surface.
    ///
    /// Mirrors C++ `HgiInterop::TransferToApp` → `CompositeToInterop`.
    ///
    /// * `src_color`      - Color AOV texture view from HGI rendering.
    /// * `src_depth`      - Optional depth AOV texture to read depth values from.
    /// * `dst_view`       - The wgpu surface texture view to present color onto.
    /// * `dst_depth_view` - Optional destination depth buffer attachment.
    ///                      When both `src_depth` and `dst_depth_view` are provided,
    ///                      the shader reads depth from `src_depth` and writes
    ///                      `frag_depth` to `dst_depth_view` with LessEqual testing.
    /// * `dst_format`     - Format of the destination surface texture.
    /// * `viewport`       - (x, y, w, h) viewport region in pixels.
    pub fn composite(
        &mut self,
        src_color: &wgpu::TextureView,
        src_depth: Option<&wgpu::TextureView>,
        dst_view: &wgpu::TextureView,
        dst_depth_view: Option<&wgpu::TextureView>,
        dst_format: wgpu::TextureFormat,
        viewport: [f32; 4],
    ) {
        self.wgpu.composite(
            src_color,
            src_depth,
            dst_view,
            dst_depth_view,
            dst_format,
            viewport,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_entry_points() {
        let src = HgiInteropWgpu::composite_shader_source();
        assert!(src.contains("fn vs_main"), "missing vs_main entry point");
        assert!(src.contains("fn fs_color"), "missing fs_color entry point");
        assert!(src.contains("fn fs_depth"), "missing fs_depth entry point");
    }

    #[test]
    fn shader_bindings_match_c_reference() {
        // C++ opengl.cpp binds: colorIn at texture unit 0, depthIn at unit 1.
        // Our WGSL must have matching binding indices for the pipeline layout.
        let src = HgiInteropWgpu::composite_shader_source();
        assert!(
            src.contains("@binding(0) var color_tex"),
            "color at binding 0"
        );
        assert!(
            src.contains("@binding(1) var color_sampler"),
            "sampler at binding 1"
        );
        assert!(
            src.contains("@binding(2) var depth_tex"),
            "depth at binding 2"
        );
    }

    #[test]
    fn shader_depth_uses_texture_load() {
        // C++ reads depth via texture2D(depthIn, uv).r
        // wgpu depth textures are non-filterable, so we must use textureLoad.
        let src = HgiInteropWgpu::composite_shader_source();
        assert!(
            src.contains("textureLoad(depth_tex"),
            "depth must use textureLoad"
        );
        assert!(
            src.contains("frag_depth"),
            "must output @builtin(frag_depth)"
        );
    }

    #[test]
    fn shader_fullscreen_triangle_is_procedural() {
        // C++ uses a vertex buffer; we generate vertices procedurally from vertex_index.
        let src = HgiInteropWgpu::composite_shader_source();
        assert!(
            src.contains("vertex_index"),
            "procedural vertices via vertex_index"
        );
    }
}
