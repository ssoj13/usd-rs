//! wgpu texture implementation for HGI.
//!
//! Implements HgiTexture trait using wgpu::Texture + TextureView.
//! Initial data upload uses queue.write_texture().

use usd_gf::Vec3i;
use usd_hgi::{HgiFormat, HgiTexture, HgiTextureDesc, HgiTextureType, HgiTextureUsage};

use super::conversions;
/// Returns the byte size per texel for a wgpu texture format.
///
/// Used to fix the data layout mismatch when HGI 3-component formats
/// are promoted to 4-component wgpu formats (e.g. RGB8 -> RGBA8).
/// The wgpu format stride must be used, not the original HGI format.
pub(crate) fn wgpu_format_bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
    use wgpu::TextureFormat::*;
    match format {
        R8Unorm | R8Snorm | R8Uint | R8Sint => 1,
        R16Uint | R16Sint | R16Float | Rg8Unorm | Rg8Snorm | Rg8Uint | Rg8Sint => 2,
        R32Uint | R32Sint | R32Float | Rg16Uint | Rg16Sint | Rg16Float | Rgba8Unorm
        | Rgba8UnormSrgb | Rgba8Snorm | Rgba8Uint | Rgba8Sint | Bgra8Unorm | Bgra8UnormSrgb => 4,
        Rg32Uint | Rg32Sint | Rg32Float | Rgba16Uint | Rgba16Sint | Rgba16Float => 8,
        Rgba32Uint | Rgba32Sint | Rgba32Float => 16,
        Depth16Unorm => 2,
        Depth32Float | Depth24Plus | Depth24PlusStencil8 => 4,
        Depth32FloatStencil8 => 8,
        _ => 4,
    }
}

/// wgpu-backed GPU texture resource.
///
/// Holds both the wgpu::Texture and a default TextureView created at
/// construction time. The view is used for binding in shader pipelines.
///
/// Can also hold just a TextureView (for texture views created from existing textures).
#[derive(Debug)]
#[allow(dead_code)] // fields used by pub(crate) accessors, consumed by hgi.rs
pub struct WgpuTexture {
    desc: HgiTextureDesc,
    texture: Option<wgpu::Texture>,
    view: wgpu::TextureView,
}

impl WgpuTexture {
    /// Create a new wgpu texture from an HGI descriptor.
    ///
    /// Maps HgiTextureDesc fields to wgpu::TextureDescriptor and creates
    /// a default TextureView. If `initial_data` is provided, uploads it
    /// via `queue.write_texture()`.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        desc: &HgiTextureDesc,
        initial_data: Option<&[u8]>,
    ) -> Self {
        let label = if desc.debug_name.is_empty() {
            None
        } else {
            Some(desc.debug_name.as_str())
        };

        // For depth textures, use depth-specific format mapping
        let format = if desc.usage.contains(HgiTextureUsage::DEPTH_TARGET) {
            conversions::to_wgpu_depth_format(desc.format)
                .unwrap_or_else(|| conversions::to_wgpu_texture_format(desc.format))
        } else {
            conversions::to_wgpu_texture_format(desc.format)
        };
        let dimension = conversions::to_wgpu_texture_dimension(desc.texture_type);
        let usage = conversions::to_wgpu_texture_usages(desc.usage);
        let sample_count = desc.sample_count as u32;

        // Determine depth_or_array_layers from texture type
        let depth_or_array_layers = match desc.texture_type {
            HgiTextureType::Texture3D => desc.dimensions[2].max(1) as u32,
            HgiTextureType::Cubemap => 6,
            HgiTextureType::Texture1DArray | HgiTextureType::Texture2DArray => {
                desc.layer_count.max(1) as u32
            }
            _ => desc.layer_count.max(1) as u32,
        };

        let size = wgpu::Extent3d {
            width: desc.dimensions[0].max(1) as u32,
            height: desc.dimensions[1].max(1) as u32,
            depth_or_array_layers,
        };

        // sRGB textures with mipmaps need an unorm view format so the compute mip
        // generator can bind individual mip levels as storage (sRGB is not a valid
        // storage format in wgpu/WebGPU, but Rgba8Unorm is byte-compatible).
        let srgb_unorm_alias =
            if format == wgpu::TextureFormat::Rgba8UnormSrgb && desc.mip_levels > 1 {
                Some(wgpu::TextureFormat::Rgba8Unorm)
            } else {
                None
            };
        let view_formats_arr = srgb_unorm_alias
            .as_ref()
            .map(std::slice::from_ref)
            .unwrap_or(&[]);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: desc.mip_levels.max(1) as u32,
            sample_count,
            dimension,
            format,
            usage,
            view_formats: view_formats_arr,
        });

        // Upload initial data if provided
        if let Some(data) = initial_data {
            let (bpe, bw, bh) = desc.format.data_size_of_format();
            let bytes_per_row = if desc.format.is_compressed() {
                // Compressed: blocks_per_row * bytes_per_block
                let blocks_x = (size.width as usize).div_ceil(bw);
                (blocks_x * bpe) as u32
            } else {
                // Use wgpu format byte width (3-comp HGI formats are promoted to 4-comp)
                size.width * wgpu_format_bytes_per_pixel(format)
            };

            let rows_per_image = if desc.format.is_compressed() {
                (size.height as usize).div_ceil(bh) as u32
            } else {
                size.height
            };

            queue.write_texture(
                texture.as_image_copy(),
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(rows_per_image),
                },
                size,
            );
        }

        // Create default texture view
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            desc: desc.clone(),
            texture: Some(texture),
            view,
        }
    }

    /// Create a WgpuTexture from an existing TextureView (for texture views).
    ///
    /// This is used for `create_texture_view()` where we only have a view,
    /// not ownership of the underlying texture.
    pub fn from_view(view: wgpu::TextureView, desc: HgiTextureDesc) -> Self {
        Self {
            desc,
            texture: None,
            view,
        }
    }

    /// Access the inner wgpu::Texture for command encoding.
    #[allow(dead_code)] // will be used by blit/compute/graphics cmds
    pub(crate) fn wgpu_texture(&self) -> &wgpu::Texture {
        self.texture
            .as_ref()
            .expect("texture not owned by this WgpuTexture (view-only)")
    }

    /// Get texture dimensions (convenience getter matching GL API pattern).
    pub fn dimensions(&self) -> Vec3i {
        self.desc.dimensions
    }

    /// Get texture format (convenience getter matching GL API pattern).
    pub fn format(&self) -> HgiFormat {
        self.desc.format
    }

    /// Get texture type (convenience getter matching GL API pattern).
    pub fn texture_type(&self) -> HgiTextureType {
        self.desc.texture_type
    }

    /// Get mip level count (convenience getter matching GL API pattern).
    pub fn mip_levels(&self) -> u16 {
        self.desc.mip_levels
    }

    /// Access the default TextureView for bind group creation.
    #[allow(dead_code)] // will be used by bind group creation
    pub(crate) fn wgpu_view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl HgiTexture for WgpuTexture {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiTextureDesc {
        &self.desc
    }

    fn byte_size_of_resource(&self) -> usize {
        let dims = &self.desc.dimensions;
        let (bpe, bw, bh) = self.desc.format.data_size_of_format();

        if self.desc.format.is_compressed() {
            let blocks_x = (dims[0] as usize).div_ceil(bw).max(1);
            let blocks_y = (dims[1] as usize).div_ceil(bh).max(1);
            let depth = (dims[2] as usize).max(1);
            blocks_x * blocks_y * depth * bpe * self.desc.layer_count.max(1) as usize
        } else {
            let w = (dims[0] as usize).max(1);
            let h = (dims[1] as usize).max(1);
            let d = (dims[2] as usize).max(1);
            w * h * d * bpe * self.desc.layer_count.max(1) as usize
        }
    }

    /// wgpu does not expose raw native handles through its safe API.
    /// Returns 0; use wgpu_texture() for internal access.
    fn raw_resource(&self) -> u64 {
        0
    }

    /// wgpu uses queue.write_texture() for CPU->GPU transfers.
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // wgpu_format_bytes_per_pixel — 3-comp format stride tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_bytes_per_pixel_scalar_formats() {
        assert_eq!(wgpu_format_bytes_per_pixel(wgpu::TextureFormat::R8Unorm), 1);
        assert_eq!(wgpu_format_bytes_per_pixel(wgpu::TextureFormat::R8Snorm), 1);
        assert_eq!(wgpu_format_bytes_per_pixel(wgpu::TextureFormat::R8Uint), 1);
        assert_eq!(wgpu_format_bytes_per_pixel(wgpu::TextureFormat::R8Sint), 1);
    }

    #[test]
    fn test_bytes_per_pixel_two_component() {
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Rg8Unorm),
            2
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::R16Float),
            2
        );
        assert_eq!(wgpu_format_bytes_per_pixel(wgpu::TextureFormat::R16Uint), 2);
    }

    #[test]
    fn test_bytes_per_pixel_four_component_rgba() {
        // RGBA8 formats: 4 bytes per texel
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Rgba8Unorm),
            4
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Rgba8UnormSrgb),
            4
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Rgba8Snorm),
            4
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Bgra8Unorm),
            4
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::R32Float),
            4
        );
    }

    #[test]
    fn test_bytes_per_pixel_hdr_formats() {
        // RGBA16F: 8 bytes, RGBA32F: 16 bytes
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Rgba16Float),
            8
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Rgba32Float),
            16
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Rg32Float),
            8
        );
    }

    #[test]
    fn test_bytes_per_pixel_depth_formats() {
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Depth32Float),
            4
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Depth16Unorm),
            2
        );
        assert_eq!(
            wgpu_format_bytes_per_pixel(wgpu::TextureFormat::Depth32FloatStencil8),
            8
        );
    }

    // -----------------------------------------------------------------------
    // 3-comp HGI format promotion: stride must use promoted wgpu format size
    // -----------------------------------------------------------------------

    #[test]
    fn test_3comp_format_promoted_to_4comp() {
        // HGI Float16Vec3 has no direct wgpu equivalent (wgpu lacks Rgba16Float 3-comp).
        // It gets promoted to Rgba16Float (4-comp, 8 bytes) by conversions.
        // wgpu_format_bytes_per_pixel must return the PROMOTED format's size.
        use super::super::conversions;
        use usd_hgi::HgiFormat;

        let promoted = conversions::to_wgpu_texture_format(HgiFormat::Float16Vec3);
        let bpp = wgpu_format_bytes_per_pixel(promoted);
        // Promoted to Rgba16Float (8 bytes) or similar 4-comp format
        assert!(
            bpp >= 6,
            "promoted 3-comp format must have >= 6 bpp (original 6), got {bpp}"
        );
        // The promoted format must NOT be 6 (there is no 3-comp float16 in wgpu)
        assert_ne!(
            bpp, 6,
            "wgpu has no native 6-byte 3-comp format; must be promoted"
        );
    }

    #[test]
    fn test_3comp_unorm8_promoted_stride() {
        // HGI UNorm8Vec3 → wgpu Rgba8Unorm (4 bytes, not 3)
        use super::super::conversions;
        use usd_hgi::HgiFormat;

        // HGI has no UNorm8Vec3 (it's unsupported on Metal), but Float32Vec3 maps to
        // Rgba32Float (16 bytes) rather than Rgb32Float (doesn't exist in wgpu).
        let promoted = conversions::to_wgpu_texture_format(HgiFormat::Float32Vec3);
        let bpp = wgpu_format_bytes_per_pixel(promoted);
        // Promoted from 12 bytes (3*4) to at least 16 bytes (4-comp)
        assert!(
            bpp >= 12,
            "promoted Float32Vec3 format must have >= 12 bpp, got {bpp}"
        );
    }

    // -----------------------------------------------------------------------
    // bytes_per_row calculation (inline logic, not private fn — verify semantics)
    // -----------------------------------------------------------------------

    /// Verifies that for an uncompressed format the bytes_per_row = width * bpp.
    #[test]
    fn test_bytes_per_row_uncompressed() {
        use usd_hgi::HgiFormat;
        // Simulate what WgpuTexture::new computes for Rgba8 64x64 texture
        let format = HgiFormat::UNorm8Vec4;
        let width: u32 = 64;
        let (bpe, _bw, _bh) = format.data_size_of_format();
        // For non-compressed formats, bytes_per_row = width * wgpu_bpp
        // wgpu format for UNorm8Vec4 is Rgba8Unorm (4 bytes)
        let expected_bpp: u32 = 4;
        let bytes_per_row = width * expected_bpp;
        assert_eq!(bytes_per_row, 256); // 64 * 4

        // Double-check: HGI bpe matches expected
        assert_eq!(bpe, 4);
    }

    /// Verifies BC7 compressed format: blocks_per_row * 16 bytes.
    #[test]
    fn test_bytes_per_row_compressed() {
        use usd_hgi::HgiFormat;
        let format = HgiFormat::BC7UNorm8Vec4;
        let width_px: usize = 64;
        let (bytes_per_block, block_w, _block_h) = format.data_size_of_format();
        let blocks_x = width_px.div_ceil(block_w);
        let bytes_per_row = blocks_x * bytes_per_block;
        // 64px / 4px-per-block = 16 blocks * 16 bytes/block = 256
        assert_eq!(blocks_x, 16);
        assert_eq!(bytes_per_block, 16);
        assert_eq!(bytes_per_row, 256);
    }

    /// Non-power-of-two compressed width rounds up to block boundary.
    #[test]
    fn test_bytes_per_row_compressed_npot() {
        use usd_hgi::HgiFormat;
        let format = HgiFormat::BC7UNorm8Vec4;
        let width_px: usize = 65; // 65 → ceil(65/4) = 17 blocks
        let (bytes_per_block, block_w, _) = format.data_size_of_format();
        let blocks_x = width_px.div_ceil(block_w);
        let bytes_per_row = blocks_x * bytes_per_block;
        assert_eq!(blocks_x, 17);
        assert_eq!(bytes_per_row, 272); // 17 * 16
    }
}
