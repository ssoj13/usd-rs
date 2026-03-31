
//! HIO types for image format and data type representation.

use usd_gf::Vec3i;

/// HioFormat describes the memory format of image buffers used in Hio.
///
/// For reference, see:
/// https://www.khronos.org/registry/vulkan/specs/1.1/html/vkspec.html#VkFormat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum HioFormat {
    /// Invalid format.
    Invalid = -1,

    /// Single UNorm8 component: 1-byte value [0, 1]. Value = unorm / 255.0
    UNorm8 = 0,
    /// 2-component UNorm8 vector.
    UNorm8Vec2,
    /// 3-component UNorm8 vector.
    UNorm8Vec3,
    /// 4-component UNorm8 vector.
    UNorm8Vec4,

    /// Single SNorm8 component: 1-byte value [-1, 1]. Value = max(snorm / 127.0, -1.0)
    SNorm8,
    /// 2-component SNorm8 vector.
    SNorm8Vec2,
    /// 3-component SNorm8 vector.
    SNorm8Vec3,
    /// 4-component SNorm8 vector.
    SNorm8Vec4,

    /// Single Float16 component: 2-byte IEEE half-precision float.
    Float16,
    /// 2-component Float16 vector.
    Float16Vec2,
    /// 3-component Float16 vector.
    Float16Vec3,
    /// 4-component Float16 vector.
    Float16Vec4,

    /// Single Float32 component: 4-byte IEEE float.
    Float32,
    /// 2-component Float32 vector.
    Float32Vec2,
    /// 3-component Float32 vector.
    Float32Vec3,
    /// 4-component Float32 vector.
    Float32Vec4,

    /// Single Double64 component: 8-byte IEEE double.
    Double64,
    /// 2-component Double64 vector.
    Double64Vec2,
    /// 3-component Double64 vector.
    Double64Vec3,
    /// 4-component Double64 vector.
    Double64Vec4,

    /// Single UInt16 component: 2-byte unsigned short integer.
    UInt16,
    /// 2-component UInt16 vector.
    UInt16Vec2,
    /// 3-component UInt16 vector.
    UInt16Vec3,
    /// 4-component UInt16 vector.
    UInt16Vec4,

    /// Single Int16 component: 2-byte signed short integer.
    Int16,
    /// 2-component Int16 vector.
    Int16Vec2,
    /// 3-component Int16 vector.
    Int16Vec3,
    /// 4-component Int16 vector.
    Int16Vec4,

    /// Single UInt32 component: 4-byte unsigned integer.
    UInt32,
    /// 2-component UInt32 vector.
    UInt32Vec2,
    /// 3-component UInt32 vector.
    UInt32Vec3,
    /// 4-component UInt32 vector.
    UInt32Vec4,

    /// Single Int32 component: 4-byte signed integer.
    Int32,
    /// 2-component Int32 vector.
    Int32Vec2,
    /// 3-component Int32 vector.
    Int32Vec3,
    /// 4-component Int32 vector.
    Int32Vec4,

    /// Single UNorm8 component in sRGB color space [0, 1].
    UNorm8Srgb,
    /// 2-component UNorm8 vector in sRGB color space.
    UNorm8Vec2Srgb,
    /// 3-component UNorm8 vector in sRGB color space.
    UNorm8Vec3Srgb,
    /// 4-component UNorm8 vector in sRGB color space.
    UNorm8Vec4Srgb,

    /// BPTC compressed: 3-component, 4x4 blocks, signed float.
    BC6FloatVec3,

    /// BPTC compressed: 3-component, 4x4 blocks, unsigned float.
    BC6UFloatVec3,

    /// BPTC compressed: 4-component, 4x4 blocks, UNorm8 [0, 1].
    BC7UNorm8Vec4,

    /// BPTC compressed: 4-component, 4x4 blocks, UNorm8 sRGB [0, 1].
    BC7UNorm8Vec4Srgb,

    /// S3TC/DXT1 compressed: 4-component, 4x4 blocks, UNorm8 [0, 1].
    BC1UNorm8Vec4,

    /// S3TC/DXT5 compressed: 4-component, 4x4 blocks, UNorm8 [0, 1].
    BC3UNorm8Vec4,
}

impl HioFormat {
    /// Total number of format variants (excluding Invalid)
    pub const COUNT: usize = 46;
}

/// Available texture sampling dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HioAddressDimension {
    /// U texture coordinate (horizontal).
    U,
    /// V texture coordinate (vertical).
    V,
    /// W texture coordinate (depth for 3D textures).
    W,
}

/// Various modes used during sampling of a texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HioAddressMode {
    /// Clamp coordinates to [0, 1] range.
    ClampToEdge = 0,
    /// Mirror coordinates at edges, then clamp.
    MirrorClampToEdge,
    /// Repeat texture by wrapping coordinates.
    Repeat,
    /// Mirror texture at each repeat.
    MirrorRepeat,
    /// Clamp to border color for out-of-range coordinates.
    ClampToBorderColor,
}

/// Various color channel representation formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HioType {
    /// 8-bit unsigned integer (0-255).
    UnsignedByte,
    /// 8-bit unsigned integer in sRGB color space.
    UnsignedByteSRGB,
    /// 8-bit signed integer (-128 to 127).
    SignedByte,
    /// 16-bit unsigned integer.
    UnsignedShort,
    /// 16-bit signed integer.
    SignedShort,
    /// 32-bit unsigned integer.
    UnsignedInt,
    /// 32-bit signed integer.
    Int,
    /// 16-bit IEEE 754 half-precision float.
    HalfFloat,
    /// 32-bit IEEE 754 single-precision float.
    Float,
    /// 64-bit IEEE 754 double-precision float.
    Double,
}

impl HioType {
    /// Total number of type variants
    pub const COUNT: usize = 10;
}

/// Returns the HioFormat containing nchannels of HioType type.
pub fn get_format(nchannels: u32, hio_type: HioType, is_srgb: bool) -> HioFormat {
    if nchannels == 0 || nchannels > 4 {
        eprintln!("Invalid channel count: {}", nchannels);
        return HioFormat::Invalid;
    }

    let mut hio_type = hio_type;
    if is_srgb && matches!(hio_type, HioType::UnsignedByte) {
        hio_type = HioType::UnsignedByteSRGB;
    }

    // Format lookup table matching C++ implementation
    const FORMATS: [[HioFormat; 4]; 10] = [
        // UnsignedByte
        [
            HioFormat::UNorm8,
            HioFormat::UNorm8Vec2,
            HioFormat::UNorm8Vec3,
            HioFormat::UNorm8Vec4,
        ],
        // UnsignedByteSRGB
        [
            HioFormat::UNorm8Srgb,
            HioFormat::UNorm8Vec2Srgb,
            HioFormat::UNorm8Vec3Srgb,
            HioFormat::UNorm8Vec4Srgb,
        ],
        // SignedByte
        [
            HioFormat::SNorm8,
            HioFormat::SNorm8Vec2,
            HioFormat::SNorm8Vec3,
            HioFormat::SNorm8Vec4,
        ],
        // UnsignedShort
        [
            HioFormat::UInt16,
            HioFormat::UInt16Vec2,
            HioFormat::UInt16Vec3,
            HioFormat::UInt16Vec4,
        ],
        // SignedShort
        [
            HioFormat::Int16,
            HioFormat::Int16Vec2,
            HioFormat::Int16Vec3,
            HioFormat::Int16Vec4,
        ],
        // UnsignedInt
        [
            HioFormat::UInt32,
            HioFormat::UInt32Vec2,
            HioFormat::UInt32Vec3,
            HioFormat::UInt32Vec4,
        ],
        // Int
        [
            HioFormat::Int32,
            HioFormat::Int32Vec2,
            HioFormat::Int32Vec3,
            HioFormat::Int32Vec4,
        ],
        // HalfFloat
        [
            HioFormat::Float16,
            HioFormat::Float16Vec2,
            HioFormat::Float16Vec3,
            HioFormat::Float16Vec4,
        ],
        // Float
        [
            HioFormat::Float32,
            HioFormat::Float32Vec2,
            HioFormat::Float32Vec3,
            HioFormat::Float32Vec4,
        ],
        // Double
        [
            HioFormat::Double64,
            HioFormat::Double64Vec2,
            HioFormat::Double64Vec3,
            HioFormat::Double64Vec4,
        ],
    ];

    let type_idx = match hio_type {
        HioType::UnsignedByte => 0,
        HioType::UnsignedByteSRGB => 1,
        HioType::SignedByte => 2,
        HioType::UnsignedShort => 3,
        HioType::SignedShort => 4,
        HioType::UnsignedInt => 5,
        HioType::Int => 6,
        HioType::HalfFloat => 7,
        HioType::Float => 8,
        HioType::Double => 9,
    };

    FORMATS[type_idx][(nchannels - 1) as usize]
}

/// Return the HioType corresponding to the given HioFormat
pub fn get_hio_type(format: HioFormat) -> HioType {
    use HioFormat::*;
    match format {
        UNorm8 | UNorm8Vec2 | UNorm8Vec3 | UNorm8Vec4 | UNorm8Srgb | UNorm8Vec2Srgb
        | UNorm8Vec3Srgb | UNorm8Vec4Srgb | BC7UNorm8Vec4 | BC7UNorm8Vec4Srgb | BC1UNorm8Vec4
        | BC3UNorm8Vec4 => HioType::UnsignedByte,
        SNorm8 | SNorm8Vec2 | SNorm8Vec3 | SNorm8Vec4 => HioType::SignedByte,
        Float16 | Float16Vec2 | Float16Vec3 | Float16Vec4 => HioType::HalfFloat,
        Float32 | Float32Vec2 | Float32Vec3 | Float32Vec4 | BC6FloatVec3 | BC6UFloatVec3 => {
            HioType::Float
        }
        Double64 | Double64Vec2 | Double64Vec3 | Double64Vec4 => HioType::Double,
        UInt16 | UInt16Vec2 | UInt16Vec3 | UInt16Vec4 => HioType::UnsignedShort,
        Int16 | Int16Vec2 | Int16Vec3 | Int16Vec4 => HioType::SignedShort,
        UInt32 | UInt32Vec2 | UInt32Vec3 | UInt32Vec4 => HioType::UnsignedInt,
        Int32 | Int32Vec2 | Int32Vec3 | Int32Vec4 => HioType::Int,
        Invalid => {
            eprintln!("Unsupported HioFormat");
            HioType::UnsignedByte
        }
    }
}

/// Return the count of components (channels) in the given HioFormat.
pub fn get_component_count(format: HioFormat) -> i32 {
    use HioFormat::*;
    match format {
        UNorm8 | SNorm8 | Float16 | Float32 | Double64 | UInt16 | Int16 | UInt32 | Int32
        | UNorm8Srgb => 1,

        UNorm8Vec2 | SNorm8Vec2 | Float16Vec2 | Float32Vec2 | Double64Vec2 | UInt16Vec2
        | Int16Vec2 | UInt32Vec2 | Int32Vec2 | UNorm8Vec2Srgb => 2,

        UNorm8Vec3 | SNorm8Vec3 | Float16Vec3 | Float32Vec3 | Double64Vec3 | UInt16Vec3
        | Int16Vec3 | UInt32Vec3 | Int32Vec3 | UNorm8Vec3Srgb | BC6FloatVec3 | BC6UFloatVec3 => 3,

        UNorm8Vec4 | SNorm8Vec4 | Float16Vec4 | Float32Vec4 | Double64Vec4 | UInt16Vec4
        | Int16Vec4 | UInt32Vec4 | Int32Vec4 | UNorm8Vec4Srgb | BC7UNorm8Vec4
        | BC7UNorm8Vec4Srgb | BC1UNorm8Vec4 | BC3UNorm8Vec4 => 4,

        Invalid => {
            eprintln!("Unsupported format");
            1
        }
    }
}

/// Return the size in bytes for a component (channel) in the given HioType.
pub fn get_data_size_of_type(hio_type: HioType) -> usize {
    match hio_type {
        HioType::UnsignedByte | HioType::SignedByte | HioType::UnsignedByteSRGB => 1,
        HioType::UnsignedShort | HioType::SignedShort | HioType::HalfFloat => 2,
        HioType::UnsignedInt | HioType::Int | HioType::Float => 4,
        HioType::Double => 8,
    }
}

/// Return the size in bytes for a component (channel) in the given HioFormat.
pub fn get_data_size_of_type_from_format(format: HioFormat) -> usize {
    get_data_size_of_type(get_hio_type(format))
}

/// Returns the size of bytes per pixel for the given HioFormat.
/// If compressed, returns the block size and dimensions.
pub fn get_data_size_of_format(format: HioFormat) -> (usize, Option<(usize, usize)>) {
    use HioFormat::*;

    match format {
        // 1-byte formats
        UNorm8 | SNorm8 | UNorm8Srgb => (1, None),
        UNorm8Vec2 | SNorm8Vec2 | UNorm8Vec2Srgb => (2, None),
        UNorm8Vec3 | SNorm8Vec3 | UNorm8Vec3Srgb => (3, None),
        UNorm8Vec4 | SNorm8Vec4 | UNorm8Vec4Srgb => (4, None),

        // 2-byte formats (Float16, Int16, UInt16)
        Float16 | UInt16 | Int16 => (2, None),
        Float16Vec2 | UInt16Vec2 | Int16Vec2 => (4, None),
        Float16Vec3 | UInt16Vec3 | Int16Vec3 => (6, None),
        Float16Vec4 | UInt16Vec4 | Int16Vec4 => (8, None),

        // 4-byte formats (Float32, Int32, UInt32)
        Float32 | UInt32 | Int32 => (4, None),
        Float32Vec2 | UInt32Vec2 | Int32Vec2 => (8, None),
        Float32Vec3 | UInt32Vec3 | Int32Vec3 => (12, None),
        Float32Vec4 | UInt32Vec4 | Int32Vec4 => (16, None),

        // 8-byte formats (Double64)
        Double64 => (8, None),
        Double64Vec2 => (16, None),
        Double64Vec3 => (24, None),
        Double64Vec4 => (32, None),

        // Compressed formats - all return 16 bytes per 4x4 block
        BC6FloatVec3 | BC6UFloatVec3 | BC7UNorm8Vec4 | BC7UNorm8Vec4Srgb | BC1UNorm8Vec4
        | BC3UNorm8Vec4 => (16, Some((4, 4))),

        Invalid => {
            eprintln!("Unsupported format");
            (0, None)
        }
    }
}

/// Return if the given format is compressed.
pub fn is_compressed(format: HioFormat) -> bool {
    matches!(
        format,
        HioFormat::BC6FloatVec3
            | HioFormat::BC6UFloatVec3
            | HioFormat::BC7UNorm8Vec4
            | HioFormat::BC7UNorm8Vec4Srgb
            | HioFormat::BC1UNorm8Vec4
            | HioFormat::BC3UNorm8Vec4
    )
}

/// Calculate the byte size of texture. If compressed, takes block size into account.
pub fn get_data_size(format: HioFormat, dimensions: &Vec3i) -> usize {
    let (bytes_per_pixel, block_dims) = get_data_size_of_format(format);

    let num_pixels = if let Some((block_width, block_height)) = block_dims {
        // Compressed format - calculate blocks
        let blocks_x = (dimensions.x as usize).div_ceil(block_width);
        let blocks_y = (dimensions.y as usize).div_ceil(block_height);
        blocks_x * blocks_y
    } else {
        // Uncompressed format
        dimensions.x as usize * dimensions.y as usize
    };

    let depth = (dimensions.z as usize).max(1);
    num_pixels * bytes_per_pixel * depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_count() {
        assert_eq!(HioFormat::COUNT, 46);
    }

    #[test]
    fn test_get_format() {
        assert_eq!(
            get_format(1, HioType::UnsignedByte, false),
            HioFormat::UNorm8
        );
        assert_eq!(get_format(4, HioType::Float, false), HioFormat::Float32Vec4);
        assert_eq!(
            get_format(3, HioType::UnsignedByte, true),
            HioFormat::UNorm8Vec3Srgb
        );
    }

    #[test]
    fn test_get_hio_type() {
        assert_eq!(get_hio_type(HioFormat::UNorm8), HioType::UnsignedByte);
        assert_eq!(get_hio_type(HioFormat::Float32Vec4), HioType::Float);
        assert_eq!(get_hio_type(HioFormat::Int16Vec2), HioType::SignedShort);
    }

    #[test]
    fn test_component_count() {
        assert_eq!(get_component_count(HioFormat::UNorm8), 1);
        assert_eq!(get_component_count(HioFormat::Float32Vec2), 2);
        assert_eq!(get_component_count(HioFormat::UNorm8Vec3), 3);
        assert_eq!(get_component_count(HioFormat::Float32Vec4), 4);
    }

    #[test]
    fn test_data_size_of_type() {
        assert_eq!(get_data_size_of_type(HioType::UnsignedByte), 1);
        assert_eq!(get_data_size_of_type(HioType::HalfFloat), 2);
        assert_eq!(get_data_size_of_type(HioType::Float), 4);
        assert_eq!(get_data_size_of_type(HioType::Double), 8);
    }

    #[test]
    fn test_data_size_of_format() {
        assert_eq!(get_data_size_of_format(HioFormat::UNorm8), (1, None));
        assert_eq!(get_data_size_of_format(HioFormat::Float32Vec4), (16, None));
        assert_eq!(
            get_data_size_of_format(HioFormat::BC7UNorm8Vec4),
            (16, Some((4, 4)))
        );
    }

    #[test]
    fn test_is_compressed() {
        assert!(!is_compressed(HioFormat::UNorm8Vec4));
        assert!(is_compressed(HioFormat::BC6FloatVec3));
        assert!(is_compressed(HioFormat::BC7UNorm8Vec4));
    }

    #[test]
    fn test_get_data_size() {
        let dims = Vec3i::new(256, 256, 1);

        // Uncompressed: 256*256*16 (16 bytes per pixel = 4 floats * 4 bytes) = 1048576
        assert_eq!(get_data_size(HioFormat::Float32Vec4, &dims), 1048576);

        // Compressed: (256/4)*(256/4)*16 = 64*64*16 = 65536
        assert_eq!(get_data_size(HioFormat::BC7UNorm8Vec4, &dims), 65536);
    }
}
