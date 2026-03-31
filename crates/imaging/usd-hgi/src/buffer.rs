//! GPU buffer resources and descriptors

use super::enums::HgiBufferUsage;
use super::handle::HgiHandle;

/// Describes the properties needed to create a GPU buffer
#[derive(Debug, Clone)]
pub struct HgiBufferDesc {
    /// Debug label for GPU debugging
    pub debug_name: String,

    /// Bits describing the intended usage and properties of the buffer
    pub usage: HgiBufferUsage,

    /// Length of buffer in bytes
    pub byte_size: usize,

    /// The size of a vertex in a vertex buffer (only for vertex buffers)
    pub vertex_stride: u32,
    // Initial data is handled separately in Rust (not stored in descriptor)
    // Backend implementations will accept initial data during buffer creation
}

impl Default for HgiBufferDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            usage: HgiBufferUsage::UNIFORM,
            byte_size: 0,
            vertex_stride: 0,
        }
    }
}

impl HgiBufferDesc {
    /// Create a new buffer descriptor
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debug name
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Set the usage flags
    pub fn with_usage(mut self, usage: HgiBufferUsage) -> Self {
        self.usage = usage;
        self
    }

    /// Set the byte size
    pub fn with_byte_size(mut self, byte_size: usize) -> Self {
        self.byte_size = byte_size;
        self
    }

    /// Set the vertex stride
    pub fn with_vertex_stride(mut self, vertex_stride: u32) -> Self {
        self.vertex_stride = vertex_stride;
        self
    }

    /// Check if this is a valid descriptor
    pub fn is_valid(&self) -> bool {
        self.byte_size > 0 && !self.usage.is_empty()
    }
}

impl PartialEq for HgiBufferDesc {
    fn eq(&self, other: &Self) -> bool {
        self.debug_name == other.debug_name
            && self.usage == other.usage
            && self.byte_size == other.byte_size
            && self.vertex_stride == other.vertex_stride
    }
}

/// GPU buffer resource (abstract interface)
///
/// Represents a graphics platform independent GPU buffer resource.
/// Buffers should be created via Hgi::create_buffer().
pub trait HgiBuffer: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the descriptor that was used to create this buffer
    fn descriptor(&self) -> &HgiBufferDesc;

    /// Returns the byte size of the GPU buffer
    ///
    /// This can be helpful if the application wishes to tally up memory usage.
    fn byte_size_of_resource(&self) -> usize;

    /// Returns the backend's raw GPU resource handle
    ///
    /// This function returns the handle to the backend's gpu resource, cast
    /// to a uint64_t. Clients should avoid using this function and instead
    /// use HGI base classes so that client code works with any HGI platform.
    ///
    /// Platform-specific return values:
    /// - OpenGL: returns the GLuint resource name
    /// - Metal: returns the id<MTLBuffer> as u64
    /// - Vulkan: returns the VkBuffer as u64
    /// - DX12: returns the ID3D12Resource pointer as u64
    fn raw_resource(&self) -> u64;

    /// Returns the 'staging area' for CPU->GPU data transfer
    ///
    /// Some implementations (e.g. Metal) may have built-in support for
    /// queueing up CPU->GPU copies. Those implementations can return the
    /// CPU pointer to the buffer's content directly.
    ///
    /// The caller should not assume that the data from the CPU staging area
    /// is automatically flushed to the GPU. Instead, after copying is finished,
    /// the caller should use BlitCmds::copy_buffer_cpu_to_gpu() to ensure
    /// the transfer from the staging area to the GPU is scheduled.
    ///
    /// Returns None if CPU staging is not supported by the backend.
    fn cpu_staging_address(&mut self) -> Option<*mut u8>;
}

/// Type alias for buffer handle
pub type HgiBufferHandle = HgiHandle<dyn HgiBuffer>;

/// Vector of buffer handles
pub type HgiBufferHandleVector = Vec<HgiBufferHandle>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_desc_default() {
        let desc = HgiBufferDesc::default();
        assert_eq!(desc.byte_size, 0);
        assert_eq!(desc.vertex_stride, 0);
        assert!(!desc.is_valid());
    }

    #[test]
    fn test_buffer_desc_builder() {
        let desc = HgiBufferDesc::new()
            .with_debug_name("MyBuffer")
            .with_usage(HgiBufferUsage::VERTEX | HgiBufferUsage::INDEX32)
            .with_byte_size(1024)
            .with_vertex_stride(32);

        assert_eq!(desc.debug_name, "MyBuffer");
        assert!(desc.usage.contains(HgiBufferUsage::VERTEX));
        assert!(desc.usage.contains(HgiBufferUsage::INDEX32));
        assert_eq!(desc.byte_size, 1024);
        assert_eq!(desc.vertex_stride, 32);
        assert!(desc.is_valid());
    }

    #[test]
    fn test_buffer_desc_equality() {
        let desc1 = HgiBufferDesc::new()
            .with_byte_size(512)
            .with_usage(HgiBufferUsage::UNIFORM);

        let desc2 = HgiBufferDesc::new()
            .with_byte_size(512)
            .with_usage(HgiBufferUsage::UNIFORM);

        let desc3 = HgiBufferDesc::new()
            .with_byte_size(1024)
            .with_usage(HgiBufferUsage::UNIFORM);

        assert_eq!(desc1, desc2);
        assert_ne!(desc1, desc3);
    }

    // Mock implementation for testing
    struct MockBuffer {
        desc: HgiBufferDesc,
    }

    impl HgiBuffer for MockBuffer {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn descriptor(&self) -> &HgiBufferDesc {
            &self.desc
        }

        fn byte_size_of_resource(&self) -> usize {
            self.desc.byte_size
        }

        fn raw_resource(&self) -> u64 {
            0
        }

        fn cpu_staging_address(&mut self) -> Option<*mut u8> {
            None
        }
    }

    #[test]
    fn test_buffer_trait() {
        let desc = HgiBufferDesc::new().with_byte_size(256);
        let buffer = MockBuffer { desc: desc.clone() };

        assert_eq!(buffer.descriptor().byte_size, 256);
        assert_eq!(buffer.byte_size_of_resource(), 256);
    }
}
