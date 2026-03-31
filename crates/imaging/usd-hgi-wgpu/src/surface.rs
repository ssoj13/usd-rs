//! Surface integration helpers for wgpu-based windowing frameworks.
//!
//! Provides utilities for efficient pixel transfer between HgiWgpu's
//! offscreen render targets and external presentation surfaces (e.g. eframe).
//! Uses persistent staging buffers and direct texture writes to minimize
//! per-frame allocations.

use usd_hgi::texture::HgiTextureHandle;

use crate::texture::WgpuTexture;

/// Persistent staging buffer for GPU->CPU readback.
///
/// Avoids per-frame allocation by reusing a mapped buffer across frames.
/// The buffer is only reallocated when the viewport size changes.
pub struct StagingReadback {
    /// Staging buffer for GPU->CPU copy
    buffer: Option<wgpu::Buffer>,
    /// Current buffer size in bytes
    buffer_size: usize,
    /// Cached pixel data from last completed readback
    pixels: Vec<u8>,
    /// Viewport dimensions
    width: u32,
    height: u32,
    /// Bytes per pixel derived from the texture format (not hardcoded to 4)
    bytes_per_pixel: u32,
}

impl StagingReadback {
    /// Create a new staging readback manager.
    pub fn new() -> Self {
        Self {
            buffer: None,
            buffer_size: 0,
            pixels: Vec::new(),
            width: 0,
            height: 0,
            bytes_per_pixel: 4, // default RGBA8
        }
    }

    /// Read pixels from an HGI texture handle using a persistent staging buffer.
    ///
    /// Uses wgpu command encoder + copy_texture_to_buffer + buffer.slice().map_async()
    /// for efficient readback. The staging buffer is reused across frames.
    ///
    /// Returns (RGBA u8 slice, width, height) or None if readback fails.
    pub fn readback(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_handle: &HgiTextureHandle,
    ) -> Option<(&[u8], u32, u32)> {
        let wgpu_tex = texture_handle
            .get()?
            .as_any()
            .downcast_ref::<WgpuTexture>()?;

        let texture = wgpu_tex.wgpu_texture();

        // Safety: catch wgpu panics from stale/destroyed textures
        let size = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| texture.size()));
        let size = match size {
            Ok(s) => s,
            Err(_) => {
                log::warn!("[staging] readback: texture invalid (destroyed?), skipping");
                return None;
            }
        };
        let w = size.width;
        let h = size.height;

        // Derive actual bytes per pixel from the wgpu texture format
        let fmt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| texture.format()))
            .unwrap_or(wgpu::TextureFormat::Rgba8UnormSrgb);
        let bpp = crate::texture::wgpu_format_bytes_per_pixel(fmt);
        self.bytes_per_pixel = bpp;
        let bytes_per_row = Self::aligned_bytes_per_row_bpp(w, bpp);
        let needed = (bytes_per_row * h) as usize;

        // Reallocate staging buffer if size changed
        if self.buffer.is_none() || self.buffer_size < needed {
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("staging_readback"),
                size: needed as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
            self.buffer_size = needed;
            self.width = w;
            self.height = h;
        }

        let buf = self.buffer.as_ref()?;

        // Encode copy command — wrap in catch_unwind to survive stale texture IDs
        let copy_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback_encoder"),
            });

            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(h),
                    },
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );

            queue.submit(std::iter::once(encoder.finish()));
        }));

        if copy_result.is_err() {
            log::warn!(
                "[staging] readback: copy_texture_to_buffer failed (stale texture?), resetting"
            );
            // Reset staging state so we don't spam panics every frame
            self.buffer = None;
            self.buffer_size = 0;
            self.width = 0;
            self.height = 0;
            return None;
        }

        // Synchronous map (blocking) -- still faster than per-frame Vec allocation
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        if rx.recv().ok()?.is_err() {
            return None;
        }

        // Copy from mapped buffer to persistent pixel vec (strip row padding)
        let mapped = slice.get_mapped_range();
        let row_bytes = (w * self.bytes_per_pixel) as usize;
        self.pixels.resize(row_bytes * h as usize, 0);
        self.width = w;
        self.height = h;

        if bytes_per_row as usize == row_bytes {
            // No padding, direct copy
            self.pixels
                .copy_from_slice(&mapped[..row_bytes * h as usize]);
        } else {
            // Strip row alignment padding
            for y in 0..h as usize {
                let src_start = y * bytes_per_row as usize;
                let dst_start = y * row_bytes;
                self.pixels[dst_start..dst_start + row_bytes]
                    .copy_from_slice(&mapped[src_start..src_start + row_bytes]);
            }
        }

        drop(mapped);
        buf.unmap();

        Some((&self.pixels, self.width, self.height))
    }

    /// Get cached pixel data from last readback.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Get cached dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Bytes per row aligned to wgpu's COPY_BYTES_PER_ROW_ALIGNMENT (256).
    #[allow(dead_code)]
    fn aligned_bytes_per_row(width: u32) -> u32 {
        Self::aligned_bytes_per_row_bpp(width, 4)
    }

    /// Bytes per row aligned to 256, with explicit bytes-per-pixel.
    fn aligned_bytes_per_row_bpp(width: u32, bpp: u32) -> u32 {
        let unaligned = width * bpp;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        (unaligned + align - 1) / align * align
    }
}

impl Default for StagingReadback {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the wgpu TextureView from an HGI texture handle.
///
/// Useful for registering as a native texture in egui-wgpu.
/// Returns None if the handle is null or not a WgpuTexture.
pub fn get_texture_view(handle: &HgiTextureHandle) -> Option<&wgpu::TextureView> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuTexture>()
        .map(|t| t.wgpu_view())
}

/// Write pixel data directly to a pre-created wgpu texture.
///
/// Bypasses egui's ColorImage and directly writes RGBA pixels to
/// a wgpu texture that can be registered as a native egui texture.
/// Much faster than the ColorImage -> load_texture path.
pub fn write_pixels_to_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    pixels: &[u8],
    width: u32,
    height: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

/// Create a texture on the given device suitable for egui native texture registration.
///
/// Format is RGBA8 Unorm, usable as both a copy destination and texture binding.
pub fn create_presentation_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
