//! wgpu blit (copy) command buffer

use std::sync::Arc;
use usd_hgi::blit_cmds::*;
use usd_hgi::buffer::HgiBufferHandle;
use usd_hgi::cmds::HgiCmds;
use usd_hgi::texture::{HgiTexture, HgiTextureHandle};

use crate::buffer::WgpuBuffer;
use crate::conversions;
use crate::mipmap::{MipmapFormat, MipmapGenerator};
use crate::resolve;
use crate::texture::WgpuTexture;

/// Returns bytes per pixel for the actual wgpu format used on the GPU.
///
/// This is critical for 3-component HGI formats (e.g. Float32Vec3) that wgpu promotes
/// to 4-component formats (Rgba32Float). HGI bpe=12 but wgpu expects bpr = width*16.
fn wgpu_bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
    match format {
        // 8-bit formats
        wgpu::TextureFormat::R8Unorm
        | wgpu::TextureFormat::R8Snorm
        | wgpu::TextureFormat::R8Uint
        | wgpu::TextureFormat::R8Sint => 1,
        // 16-bit formats
        wgpu::TextureFormat::R16Uint
        | wgpu::TextureFormat::R16Sint
        | wgpu::TextureFormat::R16Float
        | wgpu::TextureFormat::Rg8Unorm
        | wgpu::TextureFormat::Rg8Snorm
        | wgpu::TextureFormat::Rg8Uint
        | wgpu::TextureFormat::Rg8Sint => 2,
        // 32-bit formats
        wgpu::TextureFormat::R32Uint
        | wgpu::TextureFormat::R32Sint
        | wgpu::TextureFormat::R32Float
        | wgpu::TextureFormat::Rg16Uint
        | wgpu::TextureFormat::Rg16Sint
        | wgpu::TextureFormat::Rg16Float
        | wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Rgba8Snorm
        | wgpu::TextureFormat::Rgba8Uint
        | wgpu::TextureFormat::Rgba8Sint
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb
        | wgpu::TextureFormat::Rgb10a2Unorm
        | wgpu::TextureFormat::Rg11b10Ufloat
        | wgpu::TextureFormat::Rgb9e5Ufloat => 4,
        // 64-bit formats
        wgpu::TextureFormat::Rg32Uint
        | wgpu::TextureFormat::Rg32Sint
        | wgpu::TextureFormat::Rg32Float
        | wgpu::TextureFormat::Rgba16Uint
        | wgpu::TextureFormat::Rgba16Sint
        | wgpu::TextureFormat::Rgba16Float => 8,
        // 128-bit formats
        wgpu::TextureFormat::Rgba32Uint
        | wgpu::TextureFormat::Rgba32Sint
        | wgpu::TextureFormat::Rgba32Float => 16,
        // Depth/stencil: use 4 bytes as conservative fallback
        wgpu::TextureFormat::Depth32Float => 4,
        wgpu::TextureFormat::Depth16Unorm => 2,
        wgpu::TextureFormat::Depth24Plus | wgpu::TextureFormat::Depth24PlusStencil8 => 4,
        wgpu::TextureFormat::Depth32FloatStencil8 => 8, // 4 (f32 depth) + 1 (u8 stencil) + 3 padding
        // Compressed formats: return block size (handled separately)
        _ => 4,
    }
}

/// Pending GPU->CPU readback operation.
struct PendingReadback {
    staging_buffer: wgpu::Buffer,
    destination_ptr: *mut u8,
    /// Tightly-packed bytes per row (actual pixel data, no padding)
    bytes_per_row: usize,
    /// Aligned bytes per row in the staging buffer (wgpu 256-byte alignment)
    aligned_bytes_per_row: usize,
    /// Number of rows in the image
    height: usize,
}

// SAFETY: GPU operations are synchronized and pointers are only written after device.poll
#[allow(unsafe_code)]
unsafe impl Send for PendingReadback {}
#[allow(unsafe_code)]
unsafe impl Sync for PendingReadback {}

/// wgpu blit command buffer for copy/transfer operations.
///
/// Records copy commands into a wgpu::CommandEncoder and uses Queue for
/// CPU->GPU transfers via write_buffer/write_texture.
pub struct WgpuBlitCmds {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    encoder: Option<wgpu::CommandEncoder>,
    submitted: bool,
    pending_readbacks: Vec<PendingReadback>,
    mipmap_gen: Arc<MipmapGenerator>,
}

impl WgpuBlitCmds {
    /// Create a new blit command buffer.
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        mipmap_gen: Arc<MipmapGenerator>,
    ) -> Self {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("HgiWgpu BlitCmds"),
        });
        Self {
            device,
            queue,
            encoder: Some(encoder),
            submitted: false,
            pending_readbacks: Vec::new(),
            mipmap_gen,
        }
    }
}

impl HgiCmds for WgpuBlitCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn push_debug_group(&mut self, label: &str) {
        if let Some(enc) = &mut self.encoder {
            enc.push_debug_group(label);
        }
    }

    fn pop_debug_group(&mut self) {
        if let Some(enc) = &mut self.encoder {
            enc.pop_debug_group();
        }
    }

    fn insert_debug_marker(&mut self, label: &str) {
        if let Some(enc) = &mut self.encoder {
            enc.insert_debug_marker(label);
        }
    }

    fn execute_submit(&mut self) {
        if let Some(encoder) = self.encoder.take() {
            self.queue.submit(std::iter::once(encoder.finish()));
            self.submitted = true;

            // Process pending GPU->CPU readbacks after submit
            for readback in self.pending_readbacks.drain(..) {
                let buffer_slice = readback.staging_buffer.slice(..);
                let (tx, rx) = std::sync::mpsc::channel();
                buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                    tx.send(result).ok();
                });

                // Block until mapping completes
                let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

                if let Ok(Ok(())) = rx.recv() {
                    let mapped_data = buffer_slice.get_mapped_range();
                    #[allow(unsafe_code)] // SAFETY: destination pointer guaranteed valid by caller
                    unsafe {
                        if readback.aligned_bytes_per_row == readback.bytes_per_row {
                            // No row padding -- bulk copy
                            std::ptr::copy_nonoverlapping(
                                mapped_data.as_ptr(),
                                readback.destination_ptr,
                                readback.bytes_per_row * readback.height,
                            );
                        } else {
                            // Strip 256-byte alignment padding: copy row-by-row
                            for row in 0..readback.height {
                                let src = mapped_data
                                    .as_ptr()
                                    .add(row * readback.aligned_bytes_per_row);
                                let dst =
                                    readback.destination_ptr.add(row * readback.bytes_per_row);
                                std::ptr::copy_nonoverlapping(src, dst, readback.bytes_per_row);
                            }
                        }
                    }
                    drop(mapped_data); // Release mapping before unmap
                } else {
                    log::error!("Failed to map staging buffer for GPU->CPU readback");
                }

                readback.staging_buffer.unmap();
            }
        }
    }
}

impl HgiBlitCmds for WgpuBlitCmds {
    fn copy_buffer_cpu_to_gpu(&mut self, op: &HgiBufferCpuToGpuOp) {
        if let Some(gpu_buf) = op
            .gpu_destination_buffer
            .get()
            .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>())
        {
            #[allow(unsafe_code)] // SAFETY: RawCpuBuffer guarantees validity during operation
            let src_slice =
                unsafe { std::slice::from_raw_parts(op.cpu_source_buffer.as_ptr(), op.byte_size) };

            self.queue.write_buffer(
                gpu_buf.wgpu_buffer(),
                op.destination_byte_offset as u64,
                src_slice,
            );
        } else {
            log::error!("copy_buffer_cpu_to_gpu: invalid destination buffer handle");
        }
    }

    fn copy_buffer_gpu_to_cpu(&mut self, op: &HgiBufferGpuToCpuOp) {
        if let Some(src_buf) = op
            .gpu_source_buffer
            .get()
            .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>())
        {
            let byte_size = op.byte_size as u64;

            // Create staging buffer for MAP_READ
            let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("HgiWgpu buffer GPU->CPU staging"),
                size: byte_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            if let Some(encoder) = &mut self.encoder {
                encoder.copy_buffer_to_buffer(
                    src_buf.wgpu_buffer(),
                    op.source_byte_offset as u64,
                    &staging_buffer,
                    0,
                    byte_size,
                );
            }

            // Buffer data has no row alignment padding: treat as a single flat row.
            self.pending_readbacks.push(PendingReadback {
                staging_buffer,
                destination_ptr: op.cpu_destination_buffer.as_ptr(),
                bytes_per_row: op.byte_size,
                aligned_bytes_per_row: op.byte_size,
                height: 1,
            });
        } else {
            log::error!("copy_buffer_gpu_to_cpu: invalid source buffer handle");
        }
    }

    fn copy_buffer_gpu_to_gpu(&mut self, op: &HgiBufferGpuToGpuOp) {
        if let (Some(src_buf), Some(dst_buf)) = (
            op.gpu_source_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>()),
            op.gpu_destination_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>()),
        ) {
            if let Some(encoder) = &mut self.encoder {
                encoder.copy_buffer_to_buffer(
                    src_buf.wgpu_buffer(),
                    op.source_byte_offset as u64,
                    dst_buf.wgpu_buffer(),
                    op.destination_byte_offset as u64,
                    op.byte_size as u64,
                );
            }
        } else {
            log::error!("copy_buffer_gpu_to_gpu: invalid buffer handles");
        }
    }

    fn copy_texture_cpu_to_gpu(&mut self, op: &HgiTextureCpuToGpuOp) {
        if let Some(dst_tex) = op
            .gpu_destination_texture
            .get()
            .and_then(|t| t.as_any().downcast_ref::<WgpuTexture>())
        {
            let desc = HgiTexture::descriptor(dst_tex);
            // Use the actual wgpu format for bpp — 3-comp HGI formats are promoted to 4-comp wgpu.
            // E.g. Float32Vec3 -> Rgba32Float: HGI bpe=12 but wgpu expects bpr = width*16.
            let wgpu_format = conversions::to_wgpu_texture_format(desc.format);
            let (_, bw, bh) = desc.format.data_size_of_format();

            #[allow(unsafe_code)] // SAFETY: RawCpuBuffer guarantees validity during operation
            let src_slice = unsafe {
                std::slice::from_raw_parts(op.cpu_source_buffer.as_ptr(), op.buffer_byte_size)
            };

            // Calculate bytes_per_row using wgpu format bpp (not HGI bpe) for correct row stride.
            let width = desc.dimensions[0].max(1) as u32;
            let height = desc.dimensions[1].max(1) as u32;
            let bytes_per_row = if desc.format.is_compressed() {
                let bpe_compressed = wgpu_bytes_per_pixel(wgpu_format) as usize * bw * bh;
                let blocks_x = (width as usize).div_ceil(bw);
                (blocks_x * bpe_compressed) as u32
            } else {
                width * wgpu_bytes_per_pixel(wgpu_format)
            };

            let rows_per_image = if desc.format.is_compressed() {
                (height as usize).div_ceil(bh) as u32
            } else {
                height
            };

            let copy_size = wgpu::Extent3d {
                width: desc.dimensions[0].max(1) as u32,
                height: desc.dimensions[1].max(1) as u32,
                depth_or_array_layers: 1,
            };

            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: dst_tex.wgpu_texture(),
                    mip_level: op.mip_level,
                    origin: wgpu::Origin3d {
                        x: op.destination_texel_offset[0].max(0) as u32,
                        y: op.destination_texel_offset[1].max(0) as u32,
                        z: op.destination_texel_offset[2].max(0) as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                src_slice,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(rows_per_image),
                },
                copy_size,
            );
        } else {
            log::error!("copy_texture_cpu_to_gpu: invalid texture handle");
        }
    }

    fn copy_texture_gpu_to_gpu(&mut self, op: &HgiTextureGpuToGpuOp) {
        if let (Some(src_tex), Some(dst_tex)) = (
            op.gpu_source_texture
                .get()
                .and_then(|t| t.as_any().downcast_ref::<WgpuTexture>()),
            op.gpu_destination_texture
                .get()
                .and_then(|t| t.as_any().downcast_ref::<WgpuTexture>()),
        ) {
            if let Some(encoder) = &mut self.encoder {
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: src_tex.wgpu_texture(),
                        mip_level: op.source_mip_level,
                        origin: wgpu::Origin3d {
                            x: op.source_texel_offset[0].max(0) as u32,
                            y: op.source_texel_offset[1].max(0) as u32,
                            z: op.source_texel_offset[2].max(0) as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: dst_tex.wgpu_texture(),
                        mip_level: op.destination_mip_level,
                        origin: wgpu::Origin3d {
                            x: op.destination_texel_offset[0].max(0) as u32,
                            y: op.destination_texel_offset[1].max(0) as u32,
                            z: op.destination_texel_offset[2].max(0) as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: op.copy_size[0].max(1) as u32,
                        height: op.copy_size[1].max(1) as u32,
                        depth_or_array_layers: op.copy_size[2].max(1) as u32,
                    },
                );
            }
        } else {
            log::error!("copy_texture_gpu_to_gpu: invalid texture handles");
        }
    }

    fn copy_texture_gpu_to_cpu(&mut self, op: &HgiTextureGpuToCpuOp) {
        // GPU->CPU readback requires a staging buffer + map_async.
        // We encode copy_texture_to_buffer and do a blocking map in execute_submit().
        if let Some(src_tex) = op
            .gpu_source_texture
            .get()
            .and_then(|t| t.as_any().downcast_ref::<WgpuTexture>())
        {
            let desc = HgiTexture::descriptor(src_tex);
            // Use wgpu format bpp — 3-comp HGI formats are promoted to 4-comp wgpu on GPU.
            let wgpu_format = conversions::to_wgpu_texture_format(desc.format);
            let (_, bw, bh) = desc.format.data_size_of_format();

            // Calculate buffer layout with correct wgpu row stride (not HGI bpe).
            let width = op.copy_size[0].max(1) as u32;
            let height = op.copy_size[1].max(1) as u32;
            let bytes_per_row = if desc.format.is_compressed() {
                let bpe_compressed = wgpu_bytes_per_pixel(wgpu_format) as usize * bw * bh;
                let blocks_x = (width as usize).div_ceil(bw);
                (blocks_x * bpe_compressed) as u32
            } else {
                width * wgpu_bytes_per_pixel(wgpu_format)
            };

            // wgpu requires bytes_per_row to be multiple of 256 for texture->buffer copies
            let aligned_bytes_per_row = ((bytes_per_row + 255) / 256) * 256;

            let rows_per_image = if desc.format.is_compressed() {
                (height as usize).div_ceil(bh) as u32
            } else {
                height
            };

            let buffer_size = (aligned_bytes_per_row * rows_per_image) as u64;

            // Create temporary staging buffer
            let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("HgiWgpu GPU->CPU staging"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            if let Some(encoder) = &mut self.encoder {
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: src_tex.wgpu_texture(),
                        mip_level: op.mip_level,
                        origin: wgpu::Origin3d {
                            x: op.source_texel_offset[0].max(0) as u32,
                            y: op.source_texel_offset[1].max(0) as u32,
                            z: op.source_texel_offset[2].max(0) as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &staging_buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(aligned_bytes_per_row),
                            rows_per_image: Some(rows_per_image),
                        },
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: op.copy_size[2].max(1) as u32,
                    },
                );
            }

            // Store staging buffer; execute_submit() strips row padding before writing to dst
            self.pending_readbacks.push(PendingReadback {
                staging_buffer,
                destination_ptr: op.cpu_destination_buffer.as_ptr(),
                bytes_per_row: bytes_per_row as usize,
                aligned_bytes_per_row: aligned_bytes_per_row as usize,
                height: rows_per_image as usize,
            });
        } else {
            log::error!("copy_texture_gpu_to_cpu: invalid texture handle");
        }
    }

    fn copy_buffer_to_texture(&mut self, op: &HgiBufferToTextureOp) {
        if let (Some(src_buf), Some(dst_tex)) = (
            op.gpu_source_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>()),
            op.gpu_destination_texture
                .get()
                .and_then(|t| t.as_any().downcast_ref::<WgpuTexture>()),
        ) {
            let desc = HgiTexture::descriptor(dst_tex);
            let (_, bw, bh) = desc.format.data_size_of_format();
            let wgpu_format = conversions::to_wgpu_texture_format(desc.format);

            let width = op.copy_size[0].max(1) as u32;
            let height = op.copy_size[1].max(1) as u32;
            // Use wgpu format bpp (not HGI bpe) so that promoted 4-component formats
            // (e.g. Float32Vec3 -> Rgba32Float: HGI bpe=12, wgpu bpp=16) get correct stride.
            let bytes_per_row = if desc.format.is_compressed() {
                let blocks_x = (width as usize).div_ceil(bw);
                (blocks_x * wgpu_bytes_per_pixel(wgpu_format) as usize) as u32
            } else {
                width * wgpu_bytes_per_pixel(wgpu_format)
            };

            let aligned_bytes_per_row = ((bytes_per_row + 255) / 256) * 256;

            let rows_per_image = if desc.format.is_compressed() {
                (height as usize).div_ceil(bh) as u32
            } else {
                height
            };

            if let Some(encoder) = &mut self.encoder {
                encoder.copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer: src_buf.wgpu_buffer(),
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: op.source_byte_offset as u64,
                            bytes_per_row: Some(aligned_bytes_per_row),
                            rows_per_image: Some(rows_per_image),
                        },
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: dst_tex.wgpu_texture(),
                        mip_level: op.destination_mip_level,
                        origin: wgpu::Origin3d {
                            x: op.destination_texel_offset[0].max(0) as u32,
                            y: op.destination_texel_offset[1].max(0) as u32,
                            z: op.destination_texel_offset[2].max(0) as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: op.copy_size[2].max(1) as u32,
                    },
                );
            }
        } else {
            log::error!("copy_buffer_to_texture: invalid handles");
        }
    }

    fn copy_texture_to_buffer(&mut self, op: &HgiTextureToBufferOp) {
        if let (Some(src_tex), Some(dst_buf)) = (
            op.gpu_source_texture
                .get()
                .and_then(|t| t.as_any().downcast_ref::<WgpuTexture>()),
            op.gpu_destination_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>()),
        ) {
            let desc = HgiTexture::descriptor(src_tex);
            let (_, bw, bh) = desc.format.data_size_of_format();
            let wgpu_format = conversions::to_wgpu_texture_format(desc.format);
            let width = op.copy_size[0].max(1) as u32;
            let height = op.copy_size[1].max(1) as u32;
            // Use wgpu format bpp (not HGI bpe) so that promoted 4-component formats
            // (e.g. Float32Vec3 -> Rgba32Float: HGI bpe=12, wgpu bpp=16) get correct stride.
            let bytes_per_row = if desc.format.is_compressed() {
                let blocks_x = (width as usize).div_ceil(bw);
                (blocks_x * wgpu_bytes_per_pixel(wgpu_format) as usize) as u32
            } else {
                width * wgpu_bytes_per_pixel(wgpu_format)
            };

            let aligned_bytes_per_row = ((bytes_per_row + 255) / 256) * 256;

            let rows_per_image = if desc.format.is_compressed() {
                (height as usize).div_ceil(bh) as u32
            } else {
                height
            };

            if let Some(encoder) = &mut self.encoder {
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: src_tex.wgpu_texture(),
                        mip_level: op.mip_level,
                        origin: wgpu::Origin3d {
                            x: op.source_texel_offset[0].max(0) as u32,
                            y: op.source_texel_offset[1].max(0) as u32,
                            z: op.source_texel_offset[2].max(0) as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: dst_buf.wgpu_buffer(),
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: op.destination_byte_offset as u64,
                            bytes_per_row: Some(aligned_bytes_per_row),
                            rows_per_image: Some(rows_per_image),
                        },
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: op.copy_size[2].max(1) as u32,
                    },
                );
            }
        } else {
            log::error!("copy_texture_to_buffer: invalid handles");
        }
    }

    fn generate_mipmap(&mut self, texture: &HgiTextureHandle) {
        // Resolve texture handle to WgpuTexture
        let wgpu_texture = match resolve::resolve_wgpu_texture(texture) {
            Some(tex) => tex,
            None => {
                log::error!("generate_mipmap: invalid texture handle");
                return;
            }
        };

        let desc = HgiTexture::descriptor(wgpu_texture);
        let mip_count = desc.mip_levels as u32;

        // Only process if there are multiple mip levels
        if mip_count <= 1 {
            return;
        }

        // Map wgpu format to MipmapFormat. Supports LDR (rgba8) and HDR (rgba16f/rgba32f).
        // sRGB uses a unorm storage view alias (texture was created with view_formats=[Rgba8Unorm]).
        let format = conversions::to_wgpu_texture_format(desc.format);
        let mip_fmt = match format {
            wgpu::TextureFormat::Rgba8Unorm => MipmapFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb => MipmapFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba16Float => MipmapFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba32Float => MipmapFormat::Rgba32Float,
            other => {
                log::warn!(
                    "generate_mipmap: format {:?} not supported. Skipping.",
                    other
                );
                return;
            }
        };

        // Get encoder reference
        let encoder = match &mut self.encoder {
            Some(enc) => enc,
            None => {
                log::error!("generate_mipmap: encoder already submitted");
                return;
            }
        };

        // Generate mipmaps using compute shader
        self.mipmap_gen.generate(
            &self.device,
            encoder,
            wgpu_texture.wgpu_texture(),
            mip_fmt,
            mip_count,
            desc.dimensions[0].max(1) as u32,
            desc.dimensions[1].max(1) as u32,
        );
    }

    fn fill_buffer(&mut self, buffer: &HgiBufferHandle, value: u8) {
        if let Some(gpu_buf) = buffer
            .get()
            .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>())
        {
            let size = gpu_buf.wgpu_buffer().size() as usize;
            if value == 0 {
                // Fast path: wgpu clear_buffer zeroes the entire buffer.
                // Requires offset and size to be multiples of 4.
                let aligned_size = (size / 4) * 4;
                if aligned_size > 0 {
                    if let Some(encoder) = &mut self.encoder {
                        encoder.clear_buffer(gpu_buf.wgpu_buffer(), 0, Some(aligned_size as u64));
                        return;
                    }
                }
            }

            // General case: fill with repeated byte pattern via write_buffer
            let fill_data = vec![value; size];
            self.queue
                .write_buffer(gpu_buf.wgpu_buffer(), 0, &fill_data);
        } else {
            log::error!("fill_buffer: invalid buffer handle");
        }
    }
}
