//! Resource binding descriptors for shaders

use super::buffer::HgiBufferHandle;
use super::enums::{HgiBindResourceType, HgiShaderStage};
use super::handle::HgiHandle;
use super::sampler::HgiSamplerHandle;
use super::texture::HgiTextureHandle;

/// Describes the binding information of a buffer (or array of buffers).
///
/// If there are more than one buffer, the buffers will be put in an array-of-buffers.
/// Note that different platforms have varying limits to max buffers in an array.
///
/// Vertex, index and indirect buffers are not bound to a resource set.
/// They are instead passed to the draw command.
#[derive(Debug, Clone)]
pub struct HgiBufferBindDesc {
    /// The buffer(s) to be bound.
    ///
    /// If there are more than one buffer, the buffers will be put in an array-of-buffers.
    pub buffers: Vec<HgiBufferHandle>,

    /// Offset (in bytes) where data begins from the start of each buffer.
    ///
    /// There is an offset corresponding to each buffer in `buffers`.
    pub offsets: Vec<u32>,

    /// Size (in bytes) of the range of data in each buffer to bind.
    ///
    /// There is a size corresponding to each buffer in `buffers`.
    /// If empty or the size for a buffer is zero, the entire buffer is bound.
    /// If the offset for a buffer is non-zero, then a non-zero size must also be specified.
    pub sizes: Vec<u32>,

    /// The type of buffer(s) that is to be bound.
    ///
    /// All buffers in the array must have the same type.
    pub resource_type: HgiBindResourceType,

    /// Binding location for the buffer(s).
    pub binding_index: u32,

    /// What shader stage(s) the buffer will be used in.
    pub stage_usage: HgiShaderStage,

    /// Whether the buffer binding should be writable (non-const).
    pub writable: bool,
}

impl Default for HgiBufferBindDesc {
    fn default() -> Self {
        Self {
            buffers: Vec::new(),
            offsets: Vec::new(),
            sizes: Vec::new(),
            resource_type: HgiBindResourceType::UniformBuffer,
            binding_index: 0,
            stage_usage: HgiShaderStage::VERTEX,
            writable: false,
        }
    }
}

impl HgiBufferBindDesc {
    /// Creates a new buffer binding descriptor with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the buffers to be bound.
    pub fn with_buffers(mut self, buffers: Vec<HgiBufferHandle>) -> Self {
        self.buffers = buffers;
        self
    }

    /// Adds a single buffer to be bound.
    pub fn with_buffer(mut self, buffer: HgiBufferHandle) -> Self {
        self.buffers.push(buffer);
        self
    }

    /// Sets the byte offsets for the buffers.
    pub fn with_offsets(mut self, offsets: Vec<u32>) -> Self {
        self.offsets = offsets;
        self
    }

    /// Sets the sizes (in bytes) of data ranges in each buffer.
    pub fn with_sizes(mut self, sizes: Vec<u32>) -> Self {
        self.sizes = sizes;
        self
    }

    /// Sets the resource type for the buffer binding.
    pub fn with_resource_type(mut self, resource_type: HgiBindResourceType) -> Self {
        self.resource_type = resource_type;
        self
    }

    /// Sets the binding location for the buffer(s).
    pub fn with_binding_index(mut self, index: u32) -> Self {
        self.binding_index = index;
        self
    }

    /// Sets which shader stage(s) will use the buffer.
    pub fn with_stage_usage(mut self, stage: HgiShaderStage) -> Self {
        self.stage_usage = stage;
        self
    }

    /// Sets whether the buffer binding should be writable (non-const).
    pub fn with_writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }
}

impl PartialEq for HgiBufferBindDesc {
    fn eq(&self, other: &Self) -> bool {
        self.buffers == other.buffers
            && self.offsets == other.offsets
            && self.sizes == other.sizes
            && self.resource_type == other.resource_type
            && self.binding_index == other.binding_index
            && self.stage_usage == other.stage_usage
            && self.writable == other.writable
    }
}

/// Describes the binding information of a texture (or array of textures).
///
/// If there are more than one texture, the textures will be put in an array-of-textures
/// (not texture-array). Note that different platforms have varying limits to max textures
/// in an array.
#[derive(Debug, Clone)]
pub struct HgiTextureBindDesc {
    /// The texture(s) to be bound.
    ///
    /// If there are more than one texture, the textures will be put in an array-of-textures
    /// (not texture-array).
    pub textures: Vec<HgiTextureHandle>,

    /// (Optional) The sampler(s) to be bound for each texture in `textures`.
    ///
    /// If empty, a default sampler (clamp_to_edge, linear) should be used.
    pub samplers: Vec<HgiSamplerHandle>,

    /// The type of texture resource that is to be bound.
    ///
    /// All textures in the array must have the same type.
    pub resource_type: HgiBindResourceType,

    /// Binding location for the texture(s).
    pub binding_index: u32,

    /// What shader stage(s) the texture will be used in.
    pub stage_usage: HgiShaderStage,

    /// Whether the texture binding should be writable (for storage images).
    pub writable: bool,
}

impl Default for HgiTextureBindDesc {
    fn default() -> Self {
        Self {
            textures: Vec::new(),
            samplers: Vec::new(),
            resource_type: HgiBindResourceType::CombinedSamplerImage,
            binding_index: 0,
            stage_usage: HgiShaderStage::FRAGMENT,
            writable: false,
        }
    }
}

impl HgiTextureBindDesc {
    /// Creates a new texture binding descriptor with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the textures to be bound.
    pub fn with_textures(mut self, textures: Vec<HgiTextureHandle>) -> Self {
        self.textures = textures;
        self
    }

    /// Adds a single texture to be bound.
    pub fn with_texture(mut self, texture: HgiTextureHandle) -> Self {
        self.textures.push(texture);
        self
    }

    /// Sets the samplers for the textures.
    pub fn with_samplers(mut self, samplers: Vec<HgiSamplerHandle>) -> Self {
        self.samplers = samplers;
        self
    }

    /// Adds a single sampler for the textures.
    pub fn with_sampler(mut self, sampler: HgiSamplerHandle) -> Self {
        self.samplers.push(sampler);
        self
    }

    /// Sets the resource type for the texture binding.
    pub fn with_resource_type(mut self, resource_type: HgiBindResourceType) -> Self {
        self.resource_type = resource_type;
        self
    }

    /// Sets the binding location for the texture(s).
    pub fn with_binding_index(mut self, index: u32) -> Self {
        self.binding_index = index;
        self
    }

    /// Sets which shader stage(s) will use the texture.
    pub fn with_stage_usage(mut self, stage: HgiShaderStage) -> Self {
        self.stage_usage = stage;
        self
    }

    /// Sets whether the texture binding should be writable (for storage images).
    pub fn with_writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }
}

impl PartialEq for HgiTextureBindDesc {
    fn eq(&self, other: &Self) -> bool {
        self.textures == other.textures
            && self.samplers == other.samplers
            && self.resource_type == other.resource_type
            && self.binding_index == other.binding_index
            && self.stage_usage == other.stage_usage
            && self.writable == other.writable
    }
}

/// Describes a complete set of resource bindings.
///
/// Represents a set of resources (buffers and textures) that are bound to the GPU during encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct HgiResourceBindingsDesc {
    /// Debug label for GPU debugging.
    pub debug_name: String,

    /// The buffers to be bound (e.g. uniform or shader storage).
    pub buffer_bindings: Vec<HgiBufferBindDesc>,

    /// The textures to be bound.
    pub texture_bindings: Vec<HgiTextureBindDesc>,
}

impl Default for HgiResourceBindingsDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            buffer_bindings: Vec::new(),
            texture_bindings: Vec::new(),
        }
    }
}

impl HgiResourceBindingsDesc {
    /// Creates a new resource bindings descriptor with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the debug name for GPU debugging.
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Adds a buffer binding to the descriptor.
    pub fn with_buffer_binding(mut self, binding: HgiBufferBindDesc) -> Self {
        self.buffer_bindings.push(binding);
        self
    }

    /// Adds a texture binding to the descriptor.
    pub fn with_texture_binding(mut self, binding: HgiTextureBindDesc) -> Self {
        self.texture_bindings.push(binding);
        self
    }

    /// Sets all buffer bindings at once.
    pub fn with_buffer_bindings(mut self, bindings: Vec<HgiBufferBindDesc>) -> Self {
        self.buffer_bindings = bindings;
        self
    }

    /// Sets all texture bindings at once.
    pub fn with_texture_bindings(mut self, bindings: Vec<HgiTextureBindDesc>) -> Self {
        self.texture_bindings = bindings;
        self
    }
}

/// GPU resource bindings object (abstract interface).
///
/// Represents a collection of buffers, textures and vertex attributes that will be used
/// by a command buffer object (and pipeline).
///
/// Resource bindings should be created via `Hgi::create_resource_bindings()`.
pub trait HgiResourceBindings: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns the descriptor that was used to create this resource bindings object.
    fn descriptor(&self) -> &HgiResourceBindingsDesc;

    /// Returns the backend's raw GPU resource handle.
    fn raw_resource(&self) -> u64;
}

/// Type-safe handle for GPU resource bindings objects.
pub type HgiResourceBindingsHandle = HgiHandle<dyn HgiResourceBindings>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_bind_desc() {
        let desc = HgiBufferBindDesc::new()
            .with_binding_index(0)
            .with_resource_type(HgiBindResourceType::UniformBuffer)
            .with_stage_usage(HgiShaderStage::VERTEX | HgiShaderStage::FRAGMENT);

        assert_eq!(desc.binding_index, 0);
        assert_eq!(desc.resource_type, HgiBindResourceType::UniformBuffer);
        assert!(desc.stage_usage.contains(HgiShaderStage::VERTEX));
        assert!(desc.stage_usage.contains(HgiShaderStage::FRAGMENT));
    }

    #[test]
    fn test_texture_bind_desc() {
        let desc = HgiTextureBindDesc::new()
            .with_binding_index(1)
            .with_resource_type(HgiBindResourceType::CombinedSamplerImage)
            .with_stage_usage(HgiShaderStage::FRAGMENT);

        assert_eq!(desc.binding_index, 1);
        assert_eq!(
            desc.resource_type,
            HgiBindResourceType::CombinedSamplerImage
        );
        assert_eq!(desc.stage_usage, HgiShaderStage::FRAGMENT);
    }

    #[test]
    fn test_resource_bindings_desc() {
        let buffer_bind = HgiBufferBindDesc::new()
            .with_binding_index(0)
            .with_resource_type(HgiBindResourceType::UniformBuffer);

        let texture_bind = HgiTextureBindDesc::new().with_binding_index(1);

        let desc = HgiResourceBindingsDesc::new()
            .with_debug_name("MyBindings")
            .with_buffer_binding(buffer_bind)
            .with_texture_binding(texture_bind);

        assert_eq!(desc.debug_name, "MyBindings");
        assert_eq!(desc.buffer_bindings.len(), 1);
        assert_eq!(desc.texture_bindings.len(), 1);
    }
}
