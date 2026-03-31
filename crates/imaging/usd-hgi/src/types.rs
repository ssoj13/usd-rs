//! Basic types and format definitions for HGI
//!
//! This module provides core types for the Hydra Graphics Interface (HGI),
//! including pixel format enumerations, mipmap metadata, and utility functions
//! for texture memory calculations. HGI is OpenUSD's abstraction layer for
//! graphics APIs, remaining independent of Vulkan, Metal, or OpenGL specifics.

use usd_gf::Vec3i;

/// Memory format of image buffers used in Hgi
///
/// Defines pixel format types for GPU texture operations. These formats are closely
/// aligned with Vulkan formats and allow HGI to remain independent of specific
/// graphics APIs while providing comprehensive texture format support.
///
/// # Format Categories
///
/// - **Normalized formats**: `UNorm8` and `SNorm8` variants map byte values to float ranges
/// - **Floating-point formats**: `Float16` (half-precision) and `Float32` (single-precision)
/// - **Integer formats**: `Int16`, `UInt16`, `Int32` for discrete integer data
/// - **Compressed formats**: BPTC (`BC6`, `BC7`) and S3TC/DXT (`BC1`, `BC3`) compression
/// - **Special formats**: sRGB variants for gamma correction, depth-stencil, packed formats
///
/// # Platform Support
///
/// Note that `UNorm8Vec3` and `SNorm8Vec3` are unsupported in Metal and should be avoided
/// for cross-platform compatibility.
///
/// # Reference
///
/// Corresponds to `HgiFormat` in OpenUSD's `pxr/imaging/hgi/types.h`
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiFormat {
    /// Invalid or uninitialized format
    Invalid = -1,

    /// Unsigned normalized 8-bit scalar
    ///
    /// 1-byte value representing a float between 0 and 1.
    /// Conversion formula: `float value = (unorm / 255.0)`
    UNorm8 = 0,

    /// Unsigned normalized 8-bit 2-component vector
    UNorm8Vec2,

    /// Unsigned normalized 8-bit 4-component vector
    ///
    /// Note: `UNorm8Vec3` is unsupported in Metal
    UNorm8Vec4,

    /// Signed normalized 8-bit scalar
    ///
    /// 1-byte value representing a float between -1 and 1.
    /// Conversion formula: `float value = max(snorm / 127.0, -1.0)`
    SNorm8,

    /// Signed normalized 8-bit 2-component vector
    SNorm8Vec2,

    /// Signed normalized 8-bit 4-component vector
    ///
    /// Note: `SNorm8Vec3` is unsupported in Metal
    SNorm8Vec4,

    /// IEEE 754 half-precision (16-bit) floating-point scalar
    Float16,

    /// IEEE 754 half-precision 2-component vector
    Float16Vec2,

    /// IEEE 754 half-precision 3-component vector
    Float16Vec3,

    /// IEEE 754 half-precision 4-component vector
    Float16Vec4,

    /// IEEE 754 single-precision (32-bit) floating-point scalar
    Float32,

    /// IEEE 754 single-precision 2-component vector
    Float32Vec2,

    /// IEEE 754 single-precision 3-component vector
    Float32Vec3,

    /// IEEE 754 single-precision 4-component vector
    Float32Vec4,

    /// Signed 16-bit integer scalar
    Int16,

    /// Signed 16-bit integer 2-component vector
    Int16Vec2,

    /// Signed 16-bit integer 3-component vector
    Int16Vec3,

    /// Signed 16-bit integer 4-component vector
    Int16Vec4,

    /// Unsigned 16-bit integer scalar
    UInt16,

    /// Unsigned 16-bit integer 2-component vector
    UInt16Vec2,

    /// Unsigned 16-bit integer 3-component vector
    UInt16Vec3,

    /// Unsigned 16-bit integer 4-component vector
    UInt16Vec4,

    /// Signed 32-bit integer scalar
    Int32,

    /// Signed 32-bit integer 2-component vector
    Int32Vec2,

    /// Signed 32-bit integer 3-component vector
    Int32Vec3,

    /// Signed 32-bit integer 4-component vector
    Int32Vec4,

    /// Unsigned normalized 8-bit 4-component vector with sRGB encoding
    ///
    /// Gamma compression/decompression applied on read/write operations.
    /// RGB components use sRGB color space, alpha component is linear.
    UNorm8Vec4srgb,

    /// BPTC compressed 3-component vector with signed floating-point
    ///
    /// Block compression (4x4 blocks), signed float per component.
    /// Also known as BC6H signed format.
    BC6FloatVec3,

    /// BPTC compressed 3-component vector with unsigned floating-point
    ///
    /// Block compression (4x4 blocks), unsigned float per component.
    /// Also known as BC6H unsigned format.
    BC6UFloatVec3,

    /// BPTC compressed 4-component vector with unsigned normalized 8-bit
    ///
    /// Block compression (4x4 blocks), highest quality compressed format.
    /// Also known as BC7 linear format.
    BC7UNorm8Vec4,

    /// BPTC compressed 4-component vector with unsigned normalized 8-bit and sRGB
    ///
    /// Block compression (4x4 blocks) with sRGB color space.
    /// Also known as BC7 sRGB format.
    BC7UNorm8Vec4srgb,

    /// S3TC/DXT compressed 4-component vector with 1-bit alpha
    ///
    /// Block compression (4x4 blocks), 8 bytes per block.
    /// Also known as DXT1 or BC1 format.
    BC1UNorm8Vec4,

    /// S3TC/DXT compressed 4-component vector with explicit alpha
    ///
    /// Block compression (4x4 blocks), 16 bytes per block.
    /// Also known as DXT5 or BC3 format.
    BC3UNorm8Vec4,

    /// Combined depth-stencil format
    ///
    /// 32-bit float depth + 8-bit unsigned integer stencil.
    /// `Float32` alone can be used for depth-only operations.
    Float32UInt8,

    /// Packed 32-bit format with 10-10-10-2 bit layout
    ///
    /// Three 10-bit components and one 2-bit component in 32 bits total.
    /// Useful for HDR color with minimal alpha precision.
    PackedInt1010102,

    /// 16-bit unsigned normalized depth-only format
    ///
    /// Maps to wgpu::TextureFormat::Depth16Unorm.
    /// Useful for low-precision depth buffers where memory bandwidth matters.
    PackedD16Unorm,
}

impl HgiFormat {
    /// Returns the number of components in this format
    ///
    /// # Returns
    ///
    /// - `0` for `Invalid`
    /// - `1` for scalar formats
    /// - `2` for 2-component vectors or depth-stencil
    /// - `3` for 3-component vectors
    /// - `4` for 4-component vectors
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(HgiFormat::Float32Vec3.component_count(), 3);
    /// assert_eq!(HgiFormat::UNorm8.component_count(), 1);
    /// ```
    pub fn component_count(self) -> usize {
        match self {
            Self::Invalid => 0,
            Self::UNorm8
            | Self::SNorm8
            | Self::Float16
            | Self::Float32
            | Self::Int16
            | Self::UInt16
            | Self::Int32 => 1,
            Self::UNorm8Vec2
            | Self::SNorm8Vec2
            | Self::Float16Vec2
            | Self::Float32Vec2
            | Self::Int16Vec2
            | Self::UInt16Vec2
            | Self::Int32Vec2 => 2,
            Self::Float16Vec3
            | Self::Float32Vec3
            | Self::Int16Vec3
            | Self::UInt16Vec3
            | Self::Int32Vec3
            | Self::BC6FloatVec3
            | Self::BC6UFloatVec3 => 3,
            Self::UNorm8Vec4
            | Self::SNorm8Vec4
            | Self::Float16Vec4
            | Self::Float32Vec4
            | Self::Int16Vec4
            | Self::UInt16Vec4
            | Self::Int32Vec4
            | Self::UNorm8Vec4srgb
            | Self::BC7UNorm8Vec4
            | Self::BC7UNorm8Vec4srgb
            | Self::BC1UNorm8Vec4
            | Self::BC3UNorm8Vec4
            | Self::PackedInt1010102 => 4,
            Self::Float32UInt8 => 1, // treat as a single component (matches C++)
            Self::PackedD16Unorm => 1, // depth-only, single component
        }
    }

    /// Checks if this format uses block compression
    ///
    /// Block-compressed formats (BC1, BC3, BC6, BC7) use fixed-size blocks
    /// (typically 4x4 pixels) to achieve compression ratios suitable for GPU textures.
    ///
    /// # Returns
    ///
    /// `true` for BPTC and S3TC/DXT compressed formats, `false` otherwise
    pub fn is_compressed(self) -> bool {
        matches!(
            self,
            Self::BC6FloatVec3
                | Self::BC6UFloatVec3
                | Self::BC7UNorm8Vec4
                | Self::BC7UNorm8Vec4srgb
                | Self::BC1UNorm8Vec4
                | Self::BC3UNorm8Vec4
        )
    }

    /// Checks if this format uses floating-point or normalized representation
    ///
    /// Matches C++ `HgiIsFloatFormat` which calls `HgiGetComponentBaseFormat` first.
    /// Returns `true` for normalized (UNorm8/SNorm8), floating-point (Float16/Float32),
    /// BC6 compressed, depth-stencil, and packed normalized (PackedInt1010102) formats.
    pub fn is_float(self) -> bool {
        // C++ HgiIsFloatFormat uses HgiGetComponentBaseFormat then checks the base type.
        // Base types considered float: UNorm8, SNorm8, Float16, Float32,
        // Float32UInt8, BC6FloatVec3, BC6UFloatVec3, PackedInt1010102.
        matches!(
            self.component_base_format(),
            Self::UNorm8
                | Self::SNorm8
                | Self::Float16
                | Self::Float32
                | Self::Float32UInt8
                | Self::BC6FloatVec3
                | Self::BC6UFloatVec3
                | Self::PackedInt1010102
        )
    }

    /// Returns the size of a single element and block dimensions
    ///
    /// For uncompressed formats, returns bytes per pixel and block size 1x1.
    /// For compressed formats, returns bytes per compressed block and actual block dimensions.
    ///
    /// # Returns
    ///
    /// A tuple `(bytes_per_element, block_width, block_height)` where:
    /// - `bytes_per_element`: Size in bytes of one pixel or one compressed block
    /// - `block_width`: Horizontal dimension of compression block (1 for uncompressed)
    /// - `block_height`: Vertical dimension of compression block (1 for uncompressed)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Uncompressed format: 16 bytes per pixel, 1x1 blocks
    /// let (bytes, w, h) = HgiFormat::Float32Vec4.data_size_of_format();
    /// assert_eq!((bytes, w, h), (16, 1, 1));
    ///
    /// // Compressed format: 16 bytes per block, 4x4 blocks
    /// let (bytes, w, h) = HgiFormat::BC7UNorm8Vec4.data_size_of_format();
    /// assert_eq!((bytes, w, h), (16, 4, 4));
    /// ```
    pub fn data_size_of_format(self) -> (usize, usize, usize) {
        match self {
            Self::Invalid => (0, 1, 1),

            // 8-bit formats
            Self::UNorm8 | Self::SNorm8 => (1, 1, 1),
            Self::UNorm8Vec2 | Self::SNorm8Vec2 => (2, 1, 1),
            Self::UNorm8Vec4 | Self::SNorm8Vec4 | Self::UNorm8Vec4srgb => (4, 1, 1),

            // 16-bit formats
            Self::Float16 | Self::Int16 | Self::UInt16 => (2, 1, 1),
            Self::Float16Vec2 | Self::Int16Vec2 | Self::UInt16Vec2 => (4, 1, 1),
            Self::Float16Vec3 | Self::Int16Vec3 | Self::UInt16Vec3 => (6, 1, 1),
            Self::Float16Vec4 | Self::Int16Vec4 | Self::UInt16Vec4 => (8, 1, 1),

            // 32-bit formats
            Self::Float32 | Self::Int32 | Self::PackedInt1010102 => (4, 1, 1),
            Self::Float32Vec2 | Self::Int32Vec2 => (8, 1, 1),
            Self::Float32Vec3 | Self::Int32Vec3 => (12, 1, 1),
            Self::Float32Vec4 | Self::Int32Vec4 => (16, 1, 1),

            // Depth-stencil
            Self::Float32UInt8 => (8, 1, 1),

            // 16-bit depth-only
            Self::PackedD16Unorm => (2, 1, 1),

            // BC6 compressed: 16 bytes per 4x4 block
            Self::BC6FloatVec3 | Self::BC6UFloatVec3 => (16, 4, 4),

            // BC7 compressed: 16 bytes per 4x4 block
            Self::BC7UNorm8Vec4 | Self::BC7UNorm8Vec4srgb => (16, 4, 4),

            // BC1 compressed: 16 bytes per 4x4 block (matches C++ HgiGetDataSizeOfFormat)
            Self::BC1UNorm8Vec4 => (16, 4, 4),

            // BC3 compressed: 16 bytes per 4x4 block
            Self::BC3UNorm8Vec4 => (16, 4, 4),
        }
    }

    /// Returns the scalar base format for vector formats
    ///
    /// Extracts the fundamental component type from a vector format.
    /// Scalar formats return themselves unchanged.
    ///
    /// # Returns
    ///
    /// The base scalar format (e.g., `Float32` for `Float32Vec3`)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(HgiFormat::Float32Vec3.component_base_format(), HgiFormat::Float32);
    /// assert_eq!(HgiFormat::UNorm8.component_base_format(), HgiFormat::UNorm8);
    /// ```
    pub fn component_base_format(self) -> Self {
        // C++ HgiGetComponentBaseFormat: vector formats reduce to scalar base.
        // BC7/BC1/BC3 use UNorm8 as their base component type.
        match self {
            Self::UNorm8Vec2
            | Self::UNorm8Vec4
            | Self::UNorm8Vec4srgb
            | Self::BC7UNorm8Vec4
            | Self::BC7UNorm8Vec4srgb
            | Self::BC1UNorm8Vec4
            | Self::BC3UNorm8Vec4 => Self::UNorm8,
            Self::SNorm8Vec2 | Self::SNorm8Vec4 => Self::SNorm8,
            Self::Float16Vec2 | Self::Float16Vec3 | Self::Float16Vec4 => Self::Float16,
            Self::Float32Vec2 | Self::Float32Vec3 | Self::Float32Vec4 => Self::Float32,
            Self::Int16Vec2 | Self::Int16Vec3 | Self::Int16Vec4 => Self::Int16,
            Self::UInt16Vec2 | Self::UInt16Vec3 | Self::UInt16Vec4 => Self::UInt16,
            Self::Int32Vec2 | Self::Int32Vec3 | Self::Int32Vec4 => Self::Int32,
            // Scalars and special formats return themselves
            _ => self,
        }
    }
}

/// Metadata describing a single mipmap level
///
/// Contains size, offset, and dimension information for one level in a mipmap chain.
/// Used for texture upload operations and memory layout calculations.
///
/// # Reference
///
/// Corresponds to `HgiMipInfo` in OpenUSD's `pxr/imaging/hgi/types.h`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiMipInfo {
    /// Offset in bytes from start of texture data to this mipmap level
    ///
    /// This offset accounts for all previous mip levels and layers in the texture.
    pub byte_offset: usize,

    /// Spatial dimensions of this mip level (width, height, depth)
    ///
    /// Each dimension is at least 1, with higher mip levels being progressively smaller.
    pub dimensions: Vec3i,

    /// Size in bytes of one layer at this mip level
    ///
    /// For array textures, multiply by layer count to get total size.
    /// For non-array textures, this is the total size of the mip level.
    pub byte_size_per_layer: usize,
}

impl HgiMipInfo {
    /// Creates a new mipmap level descriptor
    ///
    /// # Arguments
    ///
    /// * `byte_offset` - Byte offset from texture start to this mip level
    /// * `dimensions` - Spatial dimensions (width, height, depth) of this level
    /// * `byte_size_per_layer` - Size in bytes of one layer at this level
    pub fn new(byte_offset: usize, dimensions: Vec3i, byte_size_per_layer: usize) -> Self {
        Self {
            byte_offset,
            dimensions,
            byte_size_per_layer,
        }
    }
}

/// Calculates the total memory size for texture data
///
/// Computes the byte size necessary to allocate a buffer for the given dimensions
/// and format. For compressed formats, rounds dimensions up to the nearest block size.
///
/// # Arguments
///
/// * `format` - The pixel format of the texture
/// * `dimensions` - Texture dimensions as (width, height, depth)
///
/// # Returns
///
/// Total size in bytes required for the texture data
///
/// # Examples
///
/// ```ignore
/// use usd_gf::Vec3i;
/// use usd_hgi::HgiFormat;
///
/// // 16x16 texture with RGBA float32 = 16*16*16 = 4096 bytes
/// let size = get_data_size(HgiFormat::Float32Vec4, &Vec3i::new(16, 16, 1));
/// assert_eq!(size, 4096);
///
/// // Compressed format rounds up to 4x4 blocks
/// let size = get_data_size(HgiFormat::BC7UNorm8Vec4, &Vec3i::new(16, 16, 1));
/// assert_eq!(size, 256); // 4x4 blocks * 16 bytes/block
/// ```
///
/// # Reference
///
/// Corresponds to `HgiGetDataSize` in OpenUSD's `pxr/imaging/hgi/types.h`
pub fn get_data_size(format: HgiFormat, dimensions: &Vec3i) -> usize {
    let (bytes_per_element, block_width, block_height) = format.data_size_of_format();

    if format.is_compressed() {
        // Round up to block size
        let blocks_x = (dimensions[0] as usize).div_ceil(block_width).max(1);
        let blocks_y = (dimensions[1] as usize).div_ceil(block_height).max(1);
        let depth = (dimensions[2] as usize).max(1);

        blocks_x * blocks_y * depth * bytes_per_element
    } else {
        let width = (dimensions[0] as usize).max(1);
        let height = (dimensions[1] as usize).max(1);
        let depth = (dimensions[2] as usize).max(1);

        width * height * depth * bytes_per_element
    }
}

/// Generates mipmap metadata for a complete mipmap chain
///
/// Calculates offset, dimensions, and size information for all mipmap levels in a texture.
/// Each successive level is half the size of the previous level (minimum 1 pixel).
///
/// # Arguments
///
/// * `format` - The pixel format of the texture
/// * `dimensions` - Base level dimensions (width, height, depth)
/// * `layer_count` - Number of array layers (1 for non-array textures)
/// * `data_byte_size` - Optional constraint on total memory size
///
/// # Returns
///
/// Vector of [`HgiMipInfo`] structs, one per mipmap level, ordered from largest to smallest
///
/// # Termination
///
/// Mipmap generation stops when either:
/// - All dimensions reach 1x1x1, or
/// - Adding the next level would exceed `data_byte_size` (if specified)
///
/// # Mipmap Calculation
///
/// Each mip level's dimensions are calculated as `max(previous_dimension / 2, 1)`,
/// ensuring dimensions never drop below 1 pixel.
///
/// # Examples
///
/// ```ignore
/// use usd_gf::Vec3i;
/// use usd_hgi::HgiFormat;
///
/// // Generate full mipmap chain for 64x64 texture
/// let mips = get_mip_infos(HgiFormat::UNorm8Vec4, &Vec3i::new(64, 64, 1), 1, None);
/// // Produces 7 levels: 64, 32, 16, 8, 4, 2, 1
/// assert_eq!(mips.len(), 7);
/// ```
///
/// # Reference
///
/// Corresponds to `HgiGetMipInfos` in OpenUSD's `pxr/imaging/hgi/types.h`
pub fn get_mip_infos(
    format: HgiFormat,
    dimensions: &Vec3i,
    layer_count: usize,
    data_byte_size: Option<usize>,
) -> Vec<HgiMipInfo> {
    // Matches C++ HgiGetMipInfos logic:
    // numMips is computed from largest dimension, then we iterate up to numMips.
    // Each mip is pushed first, then byteOffset += size*layerCount,
    // then we check if byteOffset >= dataByteSize and break if so.
    let max_dim = dimensions[0].max(dimensions[1]).max(dimensions[2]) as u32;
    let num_mips = if max_dim == 0 {
        1u32
    } else {
        // C++ _ComputeNumMipLevels: find i such that 2^i > dim
        let mut i = 1u32;
        loop {
            if (1u32 << i) > max_dim {
                break i;
            }
            i += 1;
        }
    };

    let max_byte_size = data_byte_size.unwrap_or(usize::MAX);
    let mut mips = Vec::with_capacity(num_mips as usize);
    let mut current_dims = *dimensions;
    let mut byte_offset = 0usize;

    for _ in 0..num_mips {
        let mip_size = get_data_size(format, &current_dims);
        let total_size = mip_size * layer_count;

        // C++: push first, then increment offset, then check budget
        mips.push(HgiMipInfo::new(byte_offset, current_dims, mip_size));
        byte_offset += total_size;

        if byte_offset >= max_byte_size {
            break;
        }

        // Calculate next mip level (divide by 2, min 1)
        current_dims = Vec3i::new(
            (current_dims[0] / 2).max(1),
            (current_dims[1] / 2).max(1),
            (current_dims[2] / 2).max(1),
        );
    }

    mips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_properties() {
        assert_eq!(HgiFormat::Float32Vec3.component_count(), 3);
        assert_eq!(HgiFormat::UNorm8Vec4.component_count(), 4);
        assert_eq!(HgiFormat::Float32.component_count(), 1);

        assert!(HgiFormat::Float32.is_float());
        assert!(!HgiFormat::Int32.is_float());

        assert!(HgiFormat::BC6FloatVec3.is_compressed());
        assert!(!HgiFormat::Float32Vec4.is_compressed());
    }

    #[test]
    fn test_data_size() {
        // Uncompressed format
        let dims = Vec3i::new(16, 16, 1);
        let size = get_data_size(HgiFormat::Float32Vec4, &dims);
        assert_eq!(size, 16 * 16 * 16); // 16x16 pixels * 16 bytes per pixel

        // Compressed format
        let size = get_data_size(HgiFormat::BC7UNorm8Vec4, &dims);
        assert_eq!(size, 4 * 4 * 16); // 4x4 blocks * 16 bytes per block
    }

    #[test]
    fn test_mip_generation() {
        let dims = Vec3i::new(64, 64, 1);
        let mips = get_mip_infos(HgiFormat::UNorm8Vec4, &dims, 1, None);

        // Should have 7 mips: 64, 32, 16, 8, 4, 2, 1
        assert_eq!(mips.len(), 7);
        assert_eq!(mips[0].dimensions, Vec3i::new(64, 64, 1));
        assert_eq!(mips[6].dimensions, Vec3i::new(1, 1, 1));
    }

    #[test]
    fn test_format_sizes() {
        let (size, w, h) = HgiFormat::Float32Vec4.data_size_of_format();
        assert_eq!(size, 16);
        assert_eq!(w, 1);
        assert_eq!(h, 1);

        let (size, w, h) = HgiFormat::BC7UNorm8Vec4.data_size_of_format();
        assert_eq!(size, 16);
        assert_eq!(w, 4);
        assert_eq!(h, 4);
    }

    #[test]
    fn test_base_format() {
        assert_eq!(
            HgiFormat::Float32Vec3.component_base_format(),
            HgiFormat::Float32
        );
        assert_eq!(
            HgiFormat::UNorm8Vec4.component_base_format(),
            HgiFormat::UNorm8
        );
        assert_eq!(
            HgiFormat::Float32.component_base_format(),
            HgiFormat::Float32
        );
    }

    /// Test ALL format data sizes against C++ HgiGetDataSizeOfFormat
    #[test]
    fn test_all_format_data_sizes() {
        // 8-bit unorm/snorm
        assert_eq!(HgiFormat::UNorm8.data_size_of_format(), (1, 1, 1));
        assert_eq!(HgiFormat::SNorm8.data_size_of_format(), (1, 1, 1));
        assert_eq!(HgiFormat::UNorm8Vec2.data_size_of_format(), (2, 1, 1));
        assert_eq!(HgiFormat::SNorm8Vec2.data_size_of_format(), (2, 1, 1));
        assert_eq!(HgiFormat::UNorm8Vec4.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::SNorm8Vec4.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::UNorm8Vec4srgb.data_size_of_format(), (4, 1, 1));

        // 16-bit
        assert_eq!(HgiFormat::Float16.data_size_of_format(), (2, 1, 1));
        assert_eq!(HgiFormat::Int16.data_size_of_format(), (2, 1, 1));
        assert_eq!(HgiFormat::UInt16.data_size_of_format(), (2, 1, 1));
        assert_eq!(HgiFormat::Float16Vec2.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::Int16Vec2.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::UInt16Vec2.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::Float16Vec3.data_size_of_format(), (6, 1, 1));
        assert_eq!(HgiFormat::Int16Vec3.data_size_of_format(), (6, 1, 1));
        assert_eq!(HgiFormat::UInt16Vec3.data_size_of_format(), (6, 1, 1));
        assert_eq!(HgiFormat::Float16Vec4.data_size_of_format(), (8, 1, 1));
        assert_eq!(HgiFormat::Int16Vec4.data_size_of_format(), (8, 1, 1));
        assert_eq!(HgiFormat::UInt16Vec4.data_size_of_format(), (8, 1, 1));

        // 32-bit
        assert_eq!(HgiFormat::Float32.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::Int32.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::PackedInt1010102.data_size_of_format(), (4, 1, 1));
        assert_eq!(HgiFormat::Float32Vec2.data_size_of_format(), (8, 1, 1));
        assert_eq!(HgiFormat::Int32Vec2.data_size_of_format(), (8, 1, 1));
        // C++: Float32UInt8 returns 8 ("implementation dependent")
        assert_eq!(HgiFormat::Float32UInt8.data_size_of_format(), (8, 1, 1));
        assert_eq!(HgiFormat::Float32Vec3.data_size_of_format(), (12, 1, 1));
        assert_eq!(HgiFormat::Int32Vec3.data_size_of_format(), (12, 1, 1));
        assert_eq!(HgiFormat::Float32Vec4.data_size_of_format(), (16, 1, 1));
        assert_eq!(HgiFormat::Int32Vec4.data_size_of_format(), (16, 1, 1));

        // Compressed: all return 16 bytes per 4x4 block (matches C++)
        assert_eq!(HgiFormat::BC6FloatVec3.data_size_of_format(), (16, 4, 4));
        assert_eq!(HgiFormat::BC6UFloatVec3.data_size_of_format(), (16, 4, 4));
        assert_eq!(HgiFormat::BC7UNorm8Vec4.data_size_of_format(), (16, 4, 4));
        assert_eq!(
            HgiFormat::BC7UNorm8Vec4srgb.data_size_of_format(),
            (16, 4, 4)
        );
        // BC1 also returns 16 per C++ (not 8)
        assert_eq!(HgiFormat::BC1UNorm8Vec4.data_size_of_format(), (16, 4, 4));
        assert_eq!(HgiFormat::BC3UNorm8Vec4.data_size_of_format(), (16, 4, 4));
    }

    /// Test ALL format component_count against C++ HgiGetComponentCount
    #[test]
    fn test_all_format_component_counts() {
        // Count 1
        assert_eq!(HgiFormat::UNorm8.component_count(), 1);
        assert_eq!(HgiFormat::SNorm8.component_count(), 1);
        assert_eq!(HgiFormat::Float16.component_count(), 1);
        assert_eq!(HgiFormat::Float32.component_count(), 1);
        assert_eq!(HgiFormat::Int16.component_count(), 1);
        assert_eq!(HgiFormat::UInt16.component_count(), 1);
        assert_eq!(HgiFormat::Int32.component_count(), 1);
        // C++: Float32UInt8 is treated as a single component
        assert_eq!(HgiFormat::Float32UInt8.component_count(), 1);

        // Count 2
        assert_eq!(HgiFormat::UNorm8Vec2.component_count(), 2);
        assert_eq!(HgiFormat::SNorm8Vec2.component_count(), 2);
        assert_eq!(HgiFormat::Float16Vec2.component_count(), 2);
        assert_eq!(HgiFormat::Float32Vec2.component_count(), 2);
        assert_eq!(HgiFormat::Int16Vec2.component_count(), 2);
        assert_eq!(HgiFormat::UInt16Vec2.component_count(), 2);
        assert_eq!(HgiFormat::Int32Vec2.component_count(), 2);

        // Count 3
        assert_eq!(HgiFormat::Float16Vec3.component_count(), 3);
        assert_eq!(HgiFormat::Float32Vec3.component_count(), 3);
        assert_eq!(HgiFormat::Int16Vec3.component_count(), 3);
        assert_eq!(HgiFormat::UInt16Vec3.component_count(), 3);
        assert_eq!(HgiFormat::Int32Vec3.component_count(), 3);
        assert_eq!(HgiFormat::BC6FloatVec3.component_count(), 3);
        assert_eq!(HgiFormat::BC6UFloatVec3.component_count(), 3);

        // Count 4
        assert_eq!(HgiFormat::UNorm8Vec4.component_count(), 4);
        assert_eq!(HgiFormat::SNorm8Vec4.component_count(), 4);
        assert_eq!(HgiFormat::Float16Vec4.component_count(), 4);
        assert_eq!(HgiFormat::Float32Vec4.component_count(), 4);
        assert_eq!(HgiFormat::Int16Vec4.component_count(), 4);
        assert_eq!(HgiFormat::UInt16Vec4.component_count(), 4);
        assert_eq!(HgiFormat::Int32Vec4.component_count(), 4);
        assert_eq!(HgiFormat::UNorm8Vec4srgb.component_count(), 4);
        assert_eq!(HgiFormat::BC7UNorm8Vec4.component_count(), 4);
        assert_eq!(HgiFormat::BC7UNorm8Vec4srgb.component_count(), 4);
        assert_eq!(HgiFormat::BC1UNorm8Vec4.component_count(), 4);
        assert_eq!(HgiFormat::BC3UNorm8Vec4.component_count(), 4);
        assert_eq!(HgiFormat::PackedInt1010102.component_count(), 4);
    }

    /// Test is_float for all formats against C++ HgiIsFloatFormat
    #[test]
    fn test_all_format_is_float() {
        // Float formats (pure floating-point)
        assert!(HgiFormat::Float16.is_float());
        assert!(HgiFormat::Float16Vec2.is_float());
        assert!(HgiFormat::Float16Vec3.is_float());
        assert!(HgiFormat::Float16Vec4.is_float());
        assert!(HgiFormat::Float32.is_float());
        assert!(HgiFormat::Float32Vec2.is_float());
        assert!(HgiFormat::Float32Vec3.is_float());
        assert!(HgiFormat::Float32Vec4.is_float());
        assert!(HgiFormat::BC6FloatVec3.is_float());
        assert!(HgiFormat::BC6UFloatVec3.is_float());
        assert!(HgiFormat::Float32UInt8.is_float());

        // C++ also treats these as float (normalized types and packed)
        assert!(HgiFormat::UNorm8.is_float());
        assert!(HgiFormat::UNorm8Vec2.is_float());
        assert!(HgiFormat::UNorm8Vec4.is_float());
        assert!(HgiFormat::UNorm8Vec4srgb.is_float());
        assert!(HgiFormat::SNorm8.is_float());
        assert!(HgiFormat::SNorm8Vec2.is_float());
        assert!(HgiFormat::SNorm8Vec4.is_float());
        assert!(HgiFormat::PackedInt1010102.is_float());
        // BC7/BC1/BC3 base is UNorm8, so is_float returns true
        assert!(HgiFormat::BC7UNorm8Vec4.is_float());
        assert!(HgiFormat::BC7UNorm8Vec4srgb.is_float());
        assert!(HgiFormat::BC1UNorm8Vec4.is_float());
        assert!(HgiFormat::BC3UNorm8Vec4.is_float());

        // Non-float integer formats
        assert!(!HgiFormat::Int16.is_float());
        assert!(!HgiFormat::UInt16.is_float());
        assert!(!HgiFormat::Int32.is_float());
        assert!(!HgiFormat::Int16Vec2.is_float());
        assert!(!HgiFormat::Int32Vec4.is_float());
    }

    /// Test mip offsets: C++ accumulates byteOffset += byteSize * layerCount
    #[test]
    fn test_mip_offsets_and_layer_count() {
        // Single layer, power-of-2 texture
        let dims = Vec3i::new(4, 4, 1);
        let mips = get_mip_infos(HgiFormat::UNorm8Vec4, &dims, 1, None);
        // Mip 0: 4*4*4 = 64 bytes, offset=0
        // Mip 1: 2*2*4 = 16 bytes, offset=64
        // Mip 2: 1*1*4 = 4 bytes, offset=80
        assert_eq!(mips.len(), 3);
        assert_eq!(mips[0].byte_offset, 0);
        assert_eq!(mips[0].byte_size_per_layer, 64);
        assert_eq!(mips[1].byte_offset, 64);
        assert_eq!(mips[1].byte_size_per_layer, 16);
        assert_eq!(mips[2].byte_offset, 80);
        assert_eq!(mips[2].byte_size_per_layer, 4);

        // Array texture with 3 layers
        let mips3 = get_mip_infos(HgiFormat::UNorm8Vec4, &dims, 3, None);
        // Mip 0: 64 bytes, offset=0; next offset = 0 + 64*3 = 192
        // Mip 1: 16 bytes, offset=192; next offset = 192 + 16*3 = 240
        assert_eq!(mips3[0].byte_offset, 0);
        assert_eq!(mips3[0].byte_size_per_layer, 64);
        assert_eq!(mips3[1].byte_offset, 192);
        assert_eq!(mips3[1].byte_size_per_layer, 16);
    }

    /// Test mip budget cutoff matches C++ behavior
    ///
    /// C++ logic: push mip, then offset += size*layers, then if offset >= budget: break.
    #[test]
    fn test_mip_budget_cutoff() {
        let dims = Vec3i::new(4, 4, 1);
        // Budget of 64 bytes:
        // Mip0: push (offset=0,size=64), offset becomes 64, 64>=64 -> break
        // Result: 1 mip
        let mips = get_mip_infos(HgiFormat::UNorm8Vec4, &dims, 1, Some(64));
        assert_eq!(mips.len(), 1);
        assert_eq!(mips[0].byte_size_per_layer, 64);

        // Budget of 80:
        // Mip0: push, offset=64, 64<80 -> continue
        // Mip1 (2x2): size=16, push (offset=64,size=16), offset=80, 80>=80 -> break
        // Result: 2 mips
        let mips2 = get_mip_infos(HgiFormat::UNorm8Vec4, &dims, 1, Some(80));
        assert_eq!(mips2.len(), 2);
        assert_eq!(mips2[1].byte_offset, 64);
        assert_eq!(mips2[1].byte_size_per_layer, 16);

        // Budget of 81: same 2 mips (offset after mip1 = 80, 80<81, try mip2 but
        // 4x4 has numMips=3 so mip2 exists)
        let mips3 = get_mip_infos(HgiFormat::UNorm8Vec4, &dims, 1, Some(81));
        assert_eq!(mips3.len(), 3);
    }

    /// Test non-power-of-2 mip chain (from C++ docs: 37x53)
    #[test]
    fn test_npot_mip_chain() {
        let dims = Vec3i::new(37, 53, 1);
        let mips = get_mip_infos(HgiFormat::UNorm8Vec4, &dims, 1, None);
        // max(37,53)=53, so numMips = ceil(log2(53)) = 6
        assert_eq!(mips.len(), 6);
        assert_eq!(mips[0].dimensions, Vec3i::new(37, 53, 1));
        assert_eq!(mips[1].dimensions, Vec3i::new(18, 26, 1));
        assert_eq!(mips[2].dimensions, Vec3i::new(9, 13, 1));
        assert_eq!(mips[3].dimensions, Vec3i::new(4, 6, 1));
        assert_eq!(mips[4].dimensions, Vec3i::new(2, 3, 1));
        assert_eq!(mips[5].dimensions, Vec3i::new(1, 1, 1));
    }
}
