//! wgpu buffer implementation for HGI.
//!
//! Implements HgiBuffer trait using wgpu::Buffer as the backing GPU resource.
//! Unlike OpenGL, wgpu uses map_async for CPU staging, so cpu_staging_address
//! always returns None.

use usd_hgi::{HgiBuffer, HgiBufferDesc, HgiBufferUsage};
use wgpu::util::DeviceExt;

use super::conversions;

/// wgpu-backed GPU buffer resource.
///
/// Wraps a wgpu::Buffer created from HgiBufferDesc. Supports vertex, index,
/// uniform, storage, and indirect buffer types.
#[derive(Debug)]
#[allow(dead_code)] // fields used by pub(crate) accessors, consumed by hgi.rs
pub struct WgpuBuffer {
    desc: HgiBufferDesc,
    buffer: wgpu::Buffer,
}

impl WgpuBuffer {
    /// Create a new wgpu buffer from an HGI descriptor.
    ///
    /// If `initial_data` is provided, uses `create_buffer_init` to upload
    /// data immediately. Otherwise creates an empty buffer of `desc.byte_size`.
    pub fn new(device: &wgpu::Device, desc: &HgiBufferDesc, initial_data: Option<&[u8]>) -> Self {
        if desc.byte_size == 0 {
            log::error!("Buffer byte_size must be non-zero");
        }

        if desc.usage.contains(HgiBufferUsage::VERTEX) && desc.vertex_stride == 0 {
            log::warn!("Vertex buffers should have non-zero vertex_stride");
        }

        let usages = conversions::to_wgpu_buffer_usages(desc.usage);
        let label = if desc.debug_name.is_empty() {
            None
        } else {
            Some(desc.debug_name.as_str())
        };

        let buffer = match initial_data {
            Some(data) => {
                // Upload initial data via create_buffer_init (wgpu::util)
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label,
                    contents: data,
                    usage: usages,
                })
            }
            None => {
                // Create empty buffer of requested size
                device.create_buffer(&wgpu::BufferDescriptor {
                    label,
                    size: desc.byte_size as u64,
                    usage: usages,
                    mapped_at_creation: false,
                })
            }
        };

        Self {
            desc: desc.clone(),
            buffer,
        }
    }

    /// Get buffer size in bytes (convenience getter matching GL API pattern).
    pub fn byte_size(&self) -> usize {
        self.desc.byte_size
    }

    /// Access the inner wgpu::Buffer for command encoding.
    pub fn wgpu_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

impl HgiBuffer for WgpuBuffer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiBufferDesc {
        &self.desc
    }

    fn byte_size_of_resource(&self) -> usize {
        self.desc.byte_size
    }

    /// wgpu does not expose raw native handles through its safe API.
    /// Returns 0; use wgpu_buffer() for internal access instead.
    fn raw_resource(&self) -> u64 {
        0
    }

    /// wgpu uses map_async for CPU->GPU transfers, not direct staging pointers.
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        None
    }
}
