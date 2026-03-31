//! Graphics enumerations and flags for HGI
//!
//! This module defines all the enums and bitflags used throughout the HGI
//! (Hydra Graphics Interface) abstraction layer.

use bitflags::bitflags;

/// Base type for bit flags
pub type HgiBits = u32;

bitflags! {
    /// Describes what capabilities the requested device must have
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HgiDeviceCapabilities: HgiBits {
        /// The device must be capable of presenting graphics to screen
        const PRESENTATION = 1 << 0;
        /// The device can access GPU buffers using bindless handles
        const BINDLESS_BUFFERS = 1 << 1;
        /// The device can execute commands concurrently
        const CONCURRENT_DISPATCH = 1 << 2;
        /// The device shares all GPU and CPU memory
        const UNIFIED_MEMORY = 1 << 3;
        /// The device can provide built-in barycentric coordinates
        const BUILTIN_BARYCENTRICS = 1 << 4;
        /// The device can provide additional built-in shader variables
        const SHADER_DRAW_PARAMETERS = 1 << 5;
        /// The device supports multiple primitive, indirect drawing
        const MULTI_DRAW_INDIRECT = 1 << 6;
        /// The device can access GPU textures using bindless handles
        const BINDLESS_TEXTURES = 1 << 7;
        /// The device supports double precision types in shaders
        const SHADER_DOUBLE_PRECISION = 1 << 8;
        /// The device's clip space depth ranges from [-1,1]
        const DEPTH_RANGE_MINUS_ONE_TO_ONE = 1 << 9;
        /// Use CPP padding for shader language structures
        const CPP_SHADER_PADDING = 1 << 10;
        /// The device supports conservative rasterization
        const CONSERVATIVE_RASTER = 1 << 11;
        /// Supports reading back the stencil buffer from GPU to CPU
        const STENCIL_READBACK = 1 << 12;
        /// The device supports setting a custom depth range
        const CUSTOM_DEPTH_RANGE = 1 << 13;
        /// Supports Metal tessellation shaders
        const METAL_TESSELLATION = 1 << 14;
        /// The device requires workaround for base primitive offset
        const BASE_PRIMITIVE_OFFSET = 1 << 15;
        /// The device requires workaround for primitive id
        const PRIMITIVE_ID_EMULATION = 1 << 16;
        /// Indirect command buffers are supported
        const INDIRECT_COMMAND_BUFFERS = 1 << 17;
        /// Points can be natively rasterized as disks
        const ROUND_POINTS = 1 << 18;
        /// Single slot resource arrays are supported
        const SINGLE_SLOT_RESOURCE_ARRAYS = 1 << 19;
    }
}

/// Describes the kind of texture
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiTextureType {
    /// A one-dimensional texture
    Texture1D = 0,
    /// A two-dimensional texture
    Texture2D,
    /// A three-dimensional texture
    Texture3D,
    /// A cubemap texture
    Cubemap,
    /// An array of one-dimensional textures
    Texture1DArray,
    /// An array of two-dimensional textures
    Texture2DArray,
}

bitflags! {
    /// Describes how the texture will be used
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HgiTextureUsage: HgiBits {
        /// The texture is a color attachment rendered into via a render pass
        const COLOR_TARGET = 1 << 0;
        /// The texture is a depth attachment rendered into via a render pass
        const DEPTH_TARGET = 1 << 1;
        /// The texture is a stencil attachment rendered into via a render pass
        const STENCIL_TARGET = 1 << 2;
        /// The texture is sampled from in a shader (sampling)
        const SHADER_READ = 1 << 3;
        /// The texture is written into from in a shader (image store)
        const SHADER_WRITE = 1 << 4;
        /// Custom backend-specific bits can start here
        const CUSTOM_BITS_BEGIN = 1 << 5;
    }
}

/// Various modes used during sampling of a texture
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiSamplerAddressMode {
    /// Clamp coordinates to the edge of the texture
    ClampToEdge = 0,
    /// Mirror and clamp coordinates to the edge of the texture
    MirrorClampToEdge,
    /// Repeat texture coordinates
    Repeat,
    /// Mirror and repeat texture coordinates
    MirrorRepeat,
    /// Clamp coordinates to the border color
    ClampToBorderColor,
}

/// Sampler filtering modes that determine the pixel value that is returned
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiSamplerFilter {
    /// Returns the value of a single mipmap level
    Nearest = 0,
    /// Combines the values of multiple mipmap levels
    Linear = 1,
}

/// Mipmap filtering modes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiMipFilter {
    /// Texture is always sampled at mipmap level 0 (ie. max lod=0)
    NotMipmapped = 0,
    /// Returns the value of a single mipmap level
    Nearest = 1,
    /// Linear interpolates the values of up to two mipmap levels
    Linear = 2,
}

/// Border color to use for clamped texture values
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiBorderColor {
    /// Transparent black (0,0,0,0)
    TransparentBlack = 0,
    /// Opaque black (0,0,0,1)
    OpaqueBlack = 1,
    /// Opaque white (1,1,1,1)
    OpaqueWhite = 2,
}

/// Sample count for multi-sampling
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiSampleCount {
    /// 1 sample per pixel (no MSAA)
    Count1 = 1,
    /// 2 samples per pixel
    Count2 = 2,
    /// 4 samples per pixel
    Count4 = 4,
    /// 8 samples per pixel
    Count8 = 8,
    /// 16 samples per pixel
    Count16 = 16,
}

/// Describes what will happen to the attachment pixel data prior to rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiAttachmentLoadOp {
    /// All pixels are rendered to. Pixel data starts undefined
    DontCare = 0,
    /// The attachment pixel data is cleared to a specified color value
    Clear,
    /// Previous pixel data is loaded into attachment prior to rendering
    Load,
}

/// Describes what will happen to the attachment pixel data after rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiAttachmentStoreOp {
    /// Pixel data is undefined after rendering (no store cost)
    DontCare = 0,
    /// The attachment pixel data is stored in memory
    Store,
}

bitflags! {
    /// Describes the properties and usage of the buffer
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HgiBufferUsage: HgiBits {
        /// Shader uniform buffer
        const UNIFORM = 1 << 0;
        /// Topology 32 bit indices
        const INDEX32 = 1 << 1;
        /// Topology 16 bit indices
        const INDEX16 = 1 << 7;
        /// Vertex attributes
        const VERTEX = 1 << 2;
        /// Shader storage buffer / Argument buffer
        const STORAGE = 1 << 3;
        /// Indirect draw buffer
        const INDIRECT = 1 << 4;
        /// Buffer will be used to upload data
        const UPLOAD = 1 << 5;
        /// Custom backend-specific bits can start here
        const CUSTOM_BITS_BEGIN = 1 << 6;
    }
}

bitflags! {
    /// Describes the stage a shader function operates in
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct HgiShaderStage: HgiBits {
        /// Vertex Shader
        const VERTEX = 1 << 0;
        /// Fragment Shader
        const FRAGMENT = 1 << 1;
        /// Compute Shader
        const COMPUTE = 1 << 2;
        /// Tessellation Control - transforms control points before tessellator
        const TESSELLATION_CONTROL = 1 << 3;
        /// Tessellation Eval - generates surface geometry from control points
        const TESSELLATION_EVAL = 1 << 4;
        /// Geometry Shader - governs processing of Primitives
        const GEOMETRY = 1 << 5;
        /// Metal specific: computes tess factors and modifies post tess vertex data
        const POST_TESSELLATION_CONTROL = 1 << 6;
        /// Metal specific: performs tessellation and vertex processing
        const POST_TESSELLATION_VERTEX = 1 << 7;
        /// Custom backend-specific bits can start here
        const CUSTOM_BITS_BEGIN = 1 << 8;
    }
}

/// Describes the type of the resource to be bound
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiBindResourceType {
    /// Sampler only
    Sampler = 0,
    /// Image for use with sampling ops (texture without sampler)
    SampledImage,
    /// Image and sampler combined into one
    CombinedSamplerImage,
    /// Storage image used for image store/load ops (UAV)
    StorageImage,
    /// Uniform buffer (UBO)
    UniformBuffer,
    /// Shader storage buffer (SSBO)
    StorageBuffer,
    /// Tessellation factors for Metal tessellation
    TessFactors,
}

/// Controls polygon mode during rasterization
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiPolygonMode {
    /// Polygons are filled
    Fill = 0,
    /// Polygon edges are drawn as line segments
    Line,
    /// Polygon vertices are drawn as points
    Point,
}

/// Controls primitive (faces) culling
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiCullMode {
    /// No primitives are discarded
    None = 0,
    /// Front-facing primitives are discarded
    Front,
    /// Back-facing primitives are discarded
    Back,
    /// All primitives are discarded
    FrontAndBack,
}

/// Determines the front-facing orientation of a primitive (face)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiWinding {
    /// Primitives with clockwise vertex-order are front facing
    Clockwise = 0,
    /// Primitives with counter-clockwise vertex-order are front facing
    CounterClockwise,
}

/// Blend operations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiBlendOp {
    /// Add source and destination (S+D)
    Add = 0,
    /// Subtract destination from source (S-D)
    Subtract,
    /// Subtract source from destination (D-S)
    ReverseSubtract,
    /// Minimum of source and destination
    Min,
    /// Maximum of source and destination
    Max,
}

/// Blend factors
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiBlendFactor {
    /// Factor of zero (0)
    Zero = 0,
    /// Factor of one (1)
    One,
    /// Source color (Rs,Gs,Bs)
    SrcColor,
    /// One minus source color (1-Rs,1-Gs,1-Bs)
    OneMinusSrcColor,
    /// Destination color (Rd,Gd,Bd)
    DstColor,
    /// One minus destination color (1-Rd,1-Gd,1-Bd)
    OneMinusDstColor,
    /// Source alpha (As)
    SrcAlpha,
    /// One minus source alpha (1-As)
    OneMinusSrcAlpha,
    /// Destination alpha (Ad)
    DstAlpha,
    /// One minus destination alpha (1-Ad)
    OneMinusDstAlpha,
    /// Constant color (Rc,Gc,Bc)
    ConstantColor,
    /// One minus constant color (1-Rc,1-Gc,1-Bc)
    OneMinusConstantColor,
    /// Constant alpha (Ac)
    ConstantAlpha,
    /// One minus constant alpha (1-Ac)
    OneMinusConstantAlpha,
    /// Source alpha saturate min(As,1-Ad)
    SrcAlphaSaturate,
    /// Source 1 color (Rs1,Gs1,Bs1)
    Src1Color,
    /// One minus source 1 color (1-Rs1,1-Gs1,1-Bs1)
    OneMinusSrc1Color,
    /// Source 1 alpha (As1)
    Src1Alpha,
    /// One minus source 1 alpha (1-As1)
    OneMinusSrc1Alpha,
}

bitflags! {
    /// Describes whether to permit or restrict writing to color components
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HgiColorMask: HgiBits {
        /// Enable writing to red channel
        const RED = 1 << 0;
        /// Enable writing to green channel
        const GREEN = 1 << 1;
        /// Enable writing to blue channel
        const BLUE = 1 << 2;
        /// Enable writing to alpha channel
        const ALPHA = 1 << 3;
    }
}

/// Compare functions
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiCompareFunction {
    /// Comparison always fails
    Never = 0,
    /// Passes if source is less than destination
    Less,
    /// Passes if source equals destination
    Equal,
    /// Passes if source is less than or equal to destination
    LEqual,
    /// Passes if source is greater than destination
    Greater,
    /// Passes if source does not equal destination
    NotEqual,
    /// Passes if source is greater than or equal to destination
    GEqual,
    /// Comparison always passes
    Always,
}

/// Stencil operations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiStencilOp {
    /// Keep the current stencil value
    Keep = 0,
    /// Set stencil value to zero
    Zero,
    /// Replace stencil value with reference value
    Replace,
    /// Increment stencil value and clamp to maximum
    IncrementClamp,
    /// Decrement stencil value and clamp to zero
    DecrementClamp,
    /// Bitwise invert the stencil value
    Invert,
    /// Increment stencil value with wrapping
    IncrementWrap,
    /// Decrement stencil value with wrapping
    DecrementWrap,
}

/// Swizzle for a component
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiComponentSwizzle {
    /// Component is zero
    Zero = 0,
    /// Component is one
    One,
    /// Use red component
    R,
    /// Use green component
    G,
    /// Use blue component
    B,
    /// Use alpha component
    A,
}

/// What the stream of vertices being rendered represents
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiPrimitiveType {
    /// Rasterize a point at each vertex
    PointList = 0,
    /// Rasterize a line between each separate pair of vertices
    LineList,
    /// Rasterize a line between each pair of adjacent vertices
    LineStrip,
    /// Rasterize a triangle for every separate set of three vertices
    TriangleList,
    /// A user-defined number of vertices, tessellated into points/lines/triangles
    PatchList,
    /// A four-vertex encoding used to draw untriangulated quads
    LineListWithAdjacency,
}

/// Describes the rate at which vertex attributes are pulled from buffers
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiVertexBufferStepFunction {
    /// The same attribute data is used for every vertex
    Constant = 0,
    /// New attribute data is fetched for each vertex
    PerVertex,
    /// New attribute data is fetched for each instance
    PerInstance,
    /// New attribute data is fetched for each patch
    PerPatch,
    /// New attribute data is fetched for each patch control point
    PerPatchControlPoint,
    /// New attribute data is fetched for each draw in a multi-draw command
    PerDrawCommand,
}

/// Describes command submission wait behavior
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiSubmitWaitType {
    /// CPU should not wait for the GPU to finish processing the cmds
    NoWait = 0,
    /// The CPU waits ("blocked") until the GPU has consumed the cmds
    WaitUntilCompleted,
}

bitflags! {
    /// Describes what objects the memory barrier affects
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HgiMemoryBarrier: HgiBits {
        /// No barrier (no-op)
        const NONE = 0;
        /// The barrier affects all memory writes and reads
        const ALL = 1 << 0;
    }
}

/// Describes the type of shader resource binding model to use
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiBindingType {
    /// Shader declares binding as a value
    Value = 0,
    /// Shader declares binding as a uniform block value
    UniformValue,
    /// Shader declares binding as array value
    Array,
    /// Shader declares binding as uniform block array value
    UniformArray,
    /// Shader declares binding as pointer value
    Pointer,
}

/// Describes the type of parameter interpolation
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiInterpolationType {
    /// The shader input will have default interpolation
    Default = 0,
    /// The shader input will have no interpolation (flat)
    Flat,
    /// The shader input will be linearly interpolated in screen-space
    NoPerspective,
}

/// Describes the type of parameter sampling
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiSamplingType {
    /// The shader input will have default sampling
    Default = 0,
    /// The shader input will have centroid sampling
    Centroid,
    /// The shader input will have per-sample sampling
    Sample,
}

/// Describes the type of parameter storage
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiStorageType {
    /// The shader input will have default storage
    Default = 0,
    /// The shader input will have per-patch storage
    Patch,
}

/// Describes the type of texture to be used in shader gen
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiShaderTextureType {
    /// Regular texture
    Texture = 0,
    /// Shadow texture
    ShadowTexture,
    /// Array texture
    ArrayTexture,
    /// Cubemap texture
    CubemapTexture,
}

/// Specifies the dispatch method for compute encoders
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiComputeDispatch {
    /// Kernels are dispatched serially
    Serial = 0,
    /// Kernels are dispatched concurrently, if supported by the API
    Concurrent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_capabilities() {
        let caps = HgiDeviceCapabilities::PRESENTATION | HgiDeviceCapabilities::UNIFIED_MEMORY;
        assert!(caps.contains(HgiDeviceCapabilities::PRESENTATION));
        assert!(caps.contains(HgiDeviceCapabilities::UNIFIED_MEMORY));
        assert!(!caps.contains(HgiDeviceCapabilities::BINDLESS_BUFFERS));
    }

    #[test]
    fn test_texture_usage() {
        let usage = HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ;
        assert!(usage.contains(HgiTextureUsage::COLOR_TARGET));
        assert!(usage.contains(HgiTextureUsage::SHADER_READ));
        assert!(!usage.contains(HgiTextureUsage::DEPTH_TARGET));
    }

    #[test]
    fn test_buffer_usage() {
        let usage = HgiBufferUsage::VERTEX | HgiBufferUsage::INDEX32;
        assert!(usage.contains(HgiBufferUsage::VERTEX));
        assert!(usage.contains(HgiBufferUsage::INDEX32));
    }

    #[test]
    fn test_shader_stage() {
        let stages = HgiShaderStage::VERTEX | HgiShaderStage::FRAGMENT;
        assert!(stages.contains(HgiShaderStage::VERTEX));
        assert!(stages.contains(HgiShaderStage::FRAGMENT));
        assert!(!stages.contains(HgiShaderStage::COMPUTE));
    }

    #[test]
    fn test_enums() {
        assert_eq!(HgiTextureType::Texture2D as u32, 1);
        assert_eq!(HgiSampleCount::Count4 as u32, 4);
        assert_eq!(HgiCompareFunction::Less as u32, 1);
    }
}
