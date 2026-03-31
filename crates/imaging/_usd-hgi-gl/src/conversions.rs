//! Type conversions between HGI and OpenGL.
//!
//! This module provides utility functions for converting Hydra Graphics Interface (HGI)
//! types to their corresponding OpenGL constants. These conversions are essential for
//! translating platform-independent HGI rendering state into OpenGL-specific API calls.
//!
//! The conversions cover:
//! - Buffer usage and target mappings
//! - Texture types and formats (all 34 HgiFormat variants, matching C++ FORMAT_DESC[])
//! - Sampler filtering and addressing modes
//! - Blend operations and factors
//! - Depth/stencil comparison functions
//! - Rasterization state (culling, winding, polygon mode)
//!
//! # Reference
//! Based on OpenUSD's `pxr/imaging/hgiGL/conversions.cpp`

use usd_hgi::{
    HgiBlendFactor, HgiBlendOp, HgiBorderColor, HgiBufferUsage, HgiCompareFunction,
    HgiComponentSwizzle, HgiCullMode, HgiFormat, HgiMipFilter, HgiPolygonMode, HgiPrimitiveType,
    HgiSamplerAddressMode, HgiSamplerFilter, HgiShaderStage, HgiStencilOp, HgiTextureType,
    HgiTextureUsage, HgiWinding,
};

// OpenGL type aliases
pub type GLenum = u32;
pub type GLbitfield = u32;
pub type GLint = i32;
pub type GLsizei = i32;

// Buffer targets
pub const GL_ARRAY_BUFFER: GLenum = 0x8892;
pub const GL_ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
pub const GL_UNIFORM_BUFFER: GLenum = 0x8A11;
pub const GL_SHADER_STORAGE_BUFFER: GLenum = 0x90D2;

// Buffer usage hints
pub const GL_STATIC_DRAW: GLenum = 0x88E4;
pub const GL_DYNAMIC_DRAW: GLenum = 0x88E8;
pub const GL_STREAM_DRAW: GLenum = 0x88E0;
pub const GL_DYNAMIC_COPY: GLenum = 0x88EA;

// Texture targets
pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_TEXTURE_3D: GLenum = 0x806F;
pub const GL_TEXTURE_2D_ARRAY: GLenum = 0x8C1A;
pub const GL_TEXTURE_CUBE_MAP: GLenum = 0x8513;

// Common internal formats
pub const GL_RGBA8: GLenum = 0x8058;
pub const GL_RGBA16F: GLenum = 0x881A;
pub const GL_RGBA32F: GLenum = 0x8814;
pub const GL_RGB16F: GLenum = 0x881B;
pub const GL_DEPTH_COMPONENT24: GLenum = 0x81A6;
pub const GL_DEPTH_COMPONENT32F: GLenum = 0x8CAC;

// Depth/stencil formats
/// Base format for depth-only textures.
pub const GL_DEPTH_COMPONENT: GLenum = 0x1902;
/// Base format for combined depth+stencil textures.
pub const GL_DEPTH_STENCIL: GLenum = 0x84F9;
/// Packed type: 32-bit float depth + 24-bit unused + 8-bit stencil.
pub const GL_FLOAT_32_UNSIGNED_INT_24_8_REV: GLenum = 0x8DAD;
/// Internal format: 32-bit float depth + 8-bit stencil.
pub const GL_DEPTH32F_STENCIL8: GLenum = 0x8CAD;

// Packed format
pub const GL_INT_2_10_10_10_REV: GLenum = 0x8D9F;

// Base pixel formats
pub const GL_RED_INTEGER: GLenum = 0x8D94;
pub const GL_RG_INTEGER: GLenum = 0x8228;
pub const GL_RGB_INTEGER: GLenum = 0x8D98;
pub const GL_RGBA_INTEGER: GLenum = 0x8D99;

// Int16 internal formats
pub const GL_R16I: GLenum = 0x8233;
pub const GL_RG16I: GLenum = 0x8239;
pub const GL_RGB16I: GLenum = 0x8D89;
pub const GL_RGBA16I: GLenum = 0x8D88;

// UInt16 internal formats
pub const GL_R16UI: GLenum = 0x8234;
pub const GL_RG16UI: GLenum = 0x823A;
pub const GL_RGB16UI: GLenum = 0x8D77;
pub const GL_RGBA16UI: GLenum = 0x8D76;

// sRGB
pub const GL_SRGB8_ALPHA8: GLenum = 0x8C43;

// BPTC compressed (BC6/BC7)
pub const GL_COMPRESSED_RGB_BPTC_SIGNED_FLOAT: GLenum = 0x8E8E;
pub const GL_COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT: GLenum = 0x8E8F;
pub const GL_COMPRESSED_RGBA_BPTC_UNORM: GLenum = 0x8E8C;
pub const GL_COMPRESSED_SRGB_ALPHA_BPTC_UNORM: GLenum = 0x8E8D;

// S3TC compressed (BC1/BC3) — GL_EXT_texture_compression_s3tc
pub const GL_COMPRESSED_RGBA_S3TC_DXT1_EXT: GLenum = 0x83F1;
pub const GL_COMPRESSED_RGBA_S3TC_DXT5_EXT: GLenum = 0x83F3;

// Pixel types
pub const GL_SHORT: GLenum = 0x1402;
pub const GL_UNSIGNED_SHORT: GLenum = 0x1403;

// Sampler
pub const GL_LINEAR: GLenum = 0x2601;
pub const GL_NEAREST: GLenum = 0x2600;
pub const GL_LINEAR_MIPMAP_LINEAR: GLenum = 0x2703;
pub const GL_REPEAT: GLenum = 0x2901;
pub const GL_CLAMP_TO_EDGE: GLenum = 0x812F;
pub const GL_CLAMP_TO_BORDER: GLenum = 0x812D;
pub const GL_MIRRORED_REPEAT: GLenum = 0x8370;

// ---- Format descriptor -------------------------------------------------

/// GL format triple: (base_format, pixel_type, internal_format).
///
/// Mirrors the C++ `_FormatDesc` struct from `hgiGL/conversions.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlFormatDesc {
    /// OpenGL base pixel format (e.g. `GL_RGBA`, `GL_RED_INTEGER`).
    pub format: GLenum,
    /// OpenGL pixel data type (e.g. `GL_FLOAT`, `GL_UNSIGNED_BYTE`).
    pub ty: GLenum,
    /// OpenGL sized internal format (e.g. `GL_RGBA8`, `GL_R32F`).
    pub internal_format: GLenum,
}

/// Returns the GL format descriptor applying depth-target override when needed.
///
/// Mirrors C++ `HgiGLConversions::GetFormat()`: when `usage` contains
/// `DEPTH_TARGET` and `format` is `Float32`, returns `GL_DEPTH_COMPONENT` /
/// `GL_DEPTH_COMPONENT32F` instead of the regular R32F mapping.
pub fn hgi_format_to_gl_format_desc(format: HgiFormat, usage: HgiTextureUsage) -> GlFormatDesc {
    let mut desc = hgi_format_to_gl_format_desc_base(format);
    if usage.contains(HgiTextureUsage::DEPTH_TARGET) && format == HgiFormat::Float32 {
        desc.format = GL_DEPTH_COMPONENT;
        desc.internal_format = GL_DEPTH_COMPONENT32F;
    }
    desc
}

/// Base per-format GL descriptor without usage overrides.
///
/// Maps every HgiFormat variant to (format, type, internalFormat) exactly as
/// the C++ `FORMAT_DESC[]` array in `hgiGL/conversions.cpp`.
fn hgi_format_to_gl_format_desc_base(format: HgiFormat) -> GlFormatDesc {
    // Constants used inline to keep the table readable.
    const UBYTE: GLenum = 0x1401; // GL_UNSIGNED_BYTE
    const BYTE: GLenum = 0x1400; // GL_BYTE
    const HALF: GLenum = 0x140B; // GL_HALF_FLOAT
    const FLOAT: GLenum = 0x1406; // GL_FLOAT
    const INT: GLenum = 0x1404; // GL_INT
    const RED: GLenum = 0x1903; // GL_RED
    const RG: GLenum = 0x8227; // GL_RG
    const RGB: GLenum = 0x1907; // GL_RGB
    const RGBA: GLenum = 0x1908; // GL_RGBA

    match format {
        // UNorm8 — unsigned normalized 8-bit
        HgiFormat::UNorm8 => GlFormatDesc {
            format: RED,
            ty: UBYTE,
            internal_format: 0x8229,
        }, // GL_R8
        HgiFormat::UNorm8Vec2 => GlFormatDesc {
            format: RG,
            ty: UBYTE,
            internal_format: 0x822B,
        }, // GL_RG8
        HgiFormat::UNorm8Vec4 => GlFormatDesc {
            format: RGBA,
            ty: UBYTE,
            internal_format: GL_RGBA8,
        },

        // SNorm8 — signed normalized 8-bit
        HgiFormat::SNorm8 => GlFormatDesc {
            format: RED,
            ty: BYTE,
            internal_format: 0x8F94,
        }, // GL_R8_SNORM
        HgiFormat::SNorm8Vec2 => GlFormatDesc {
            format: RG,
            ty: BYTE,
            internal_format: 0x8F95,
        }, // GL_RG8_SNORM
        HgiFormat::SNorm8Vec4 => GlFormatDesc {
            format: RGBA,
            ty: BYTE,
            internal_format: 0x8F97,
        }, // GL_RGBA8_SNORM

        // Float16
        HgiFormat::Float16 => GlFormatDesc {
            format: RED,
            ty: HALF,
            internal_format: 0x822D,
        }, // GL_R16F
        HgiFormat::Float16Vec2 => GlFormatDesc {
            format: RG,
            ty: HALF,
            internal_format: 0x822F,
        }, // GL_RG16F
        HgiFormat::Float16Vec3 => GlFormatDesc {
            format: RGB,
            ty: HALF,
            internal_format: GL_RGB16F,
        },
        HgiFormat::Float16Vec4 => GlFormatDesc {
            format: RGBA,
            ty: HALF,
            internal_format: GL_RGBA16F,
        },

        // Float32
        HgiFormat::Float32 => GlFormatDesc {
            format: RED,
            ty: FLOAT,
            internal_format: 0x8236,
        }, // GL_R32F
        HgiFormat::Float32Vec2 => GlFormatDesc {
            format: RG,
            ty: FLOAT,
            internal_format: 0x8238,
        }, // GL_RG32F
        HgiFormat::Float32Vec3 => GlFormatDesc {
            format: RGB,
            ty: FLOAT,
            internal_format: 0x8815,
        }, // GL_RGB32F
        HgiFormat::Float32Vec4 => GlFormatDesc {
            format: RGBA,
            ty: FLOAT,
            internal_format: GL_RGBA32F,
        },

        // Int16 — signed 16-bit integer
        HgiFormat::Int16 => GlFormatDesc {
            format: GL_RED_INTEGER,
            ty: GL_SHORT,
            internal_format: GL_R16I,
        },
        HgiFormat::Int16Vec2 => GlFormatDesc {
            format: GL_RG_INTEGER,
            ty: GL_SHORT,
            internal_format: GL_RG16I,
        },
        HgiFormat::Int16Vec3 => GlFormatDesc {
            format: GL_RGB_INTEGER,
            ty: GL_SHORT,
            internal_format: GL_RGB16I,
        },
        HgiFormat::Int16Vec4 => GlFormatDesc {
            format: GL_RGBA_INTEGER,
            ty: GL_SHORT,
            internal_format: GL_RGBA16I,
        },

        // UInt16 — unsigned 16-bit integer
        HgiFormat::UInt16 => GlFormatDesc {
            format: GL_RED_INTEGER,
            ty: GL_UNSIGNED_SHORT,
            internal_format: GL_R16UI,
        },
        HgiFormat::UInt16Vec2 => GlFormatDesc {
            format: GL_RG_INTEGER,
            ty: GL_UNSIGNED_SHORT,
            internal_format: GL_RG16UI,
        },
        HgiFormat::UInt16Vec3 => GlFormatDesc {
            format: GL_RGB_INTEGER,
            ty: GL_UNSIGNED_SHORT,
            internal_format: GL_RGB16UI,
        },
        HgiFormat::UInt16Vec4 => GlFormatDesc {
            format: GL_RGBA_INTEGER,
            ty: GL_UNSIGNED_SHORT,
            internal_format: GL_RGBA16UI,
        },

        // Int32 — signed 32-bit integer
        HgiFormat::Int32 => GlFormatDesc {
            format: GL_RED_INTEGER,
            ty: INT,
            internal_format: 0x8D82,
        }, // GL_R32I
        HgiFormat::Int32Vec2 => GlFormatDesc {
            format: GL_RG_INTEGER,
            ty: INT,
            internal_format: 0x8D84,
        }, // GL_RG32I
        HgiFormat::Int32Vec3 => GlFormatDesc {
            format: GL_RGB_INTEGER,
            ty: INT,
            internal_format: 0x8D83,
        }, // GL_RGB32I
        HgiFormat::Int32Vec4 => GlFormatDesc {
            format: GL_RGBA_INTEGER,
            ty: INT,
            internal_format: 0x8D85,
        }, // GL_RGBA32I

        // sRGB 8-bit
        HgiFormat::UNorm8Vec4srgb => GlFormatDesc {
            format: RGBA,
            ty: UBYTE,
            internal_format: GL_SRGB8_ALPHA8,
        },

        // BPTC compressed (BC6/BC7)
        HgiFormat::BC6FloatVec3 => GlFormatDesc {
            format: RGB,
            ty: FLOAT,
            internal_format: GL_COMPRESSED_RGB_BPTC_SIGNED_FLOAT,
        },
        HgiFormat::BC6UFloatVec3 => GlFormatDesc {
            format: RGB,
            ty: FLOAT,
            internal_format: GL_COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT,
        },
        HgiFormat::BC7UNorm8Vec4 => GlFormatDesc {
            format: RGBA,
            ty: UBYTE,
            internal_format: GL_COMPRESSED_RGBA_BPTC_UNORM,
        },
        HgiFormat::BC7UNorm8Vec4srgb => GlFormatDesc {
            format: RGBA,
            ty: UBYTE,
            internal_format: GL_COMPRESSED_SRGB_ALPHA_BPTC_UNORM,
        },

        // S3TC compressed (BC1/BC3)
        HgiFormat::BC1UNorm8Vec4 => GlFormatDesc {
            format: RGBA,
            ty: UBYTE,
            internal_format: GL_COMPRESSED_RGBA_S3TC_DXT1_EXT,
        },
        HgiFormat::BC3UNorm8Vec4 => GlFormatDesc {
            format: RGBA,
            ty: UBYTE,
            internal_format: GL_COMPRESSED_RGBA_S3TC_DXT5_EXT,
        },

        // Depth+stencil packed
        HgiFormat::Float32UInt8 => GlFormatDesc {
            format: GL_DEPTH_STENCIL,
            ty: GL_FLOAT_32_UNSIGNED_INT_24_8_REV,
            internal_format: GL_DEPTH32F_STENCIL8,
        },

        // Packed 10/10/10/2 signed int — format == type per C++
        HgiFormat::PackedInt1010102 => GlFormatDesc {
            format: GL_INT_2_10_10_10_REV,
            ty: GL_INT_2_10_10_10_REV,
            internal_format: RGBA,
        },

        // 16-bit depth-only: GL_DEPTH_COMPONENT16
        HgiFormat::PackedD16Unorm => GlFormatDesc {
            format: 0x1902, // GL_DEPTH_COMPONENT
            ty: 0x8D48,     // GL_UNSIGNED_SHORT (used for 16-bit depth)
            internal_format: 0x81A5, // GL_DEPTH_COMPONENT16
        },

        HgiFormat::Invalid => GlFormatDesc {
            format: RGBA,
            ty: UBYTE,
            internal_format: GL_RGBA8,
        },
    }
}

// ---- Public conversion functions ---------------------------------------

/// Converts HGI buffer usage flags to OpenGL buffer target.
pub fn hgi_buffer_usage_to_gl_target(usage: HgiBufferUsage) -> GLenum {
    if usage.contains(HgiBufferUsage::VERTEX) {
        GL_ARRAY_BUFFER
    } else if usage.contains(HgiBufferUsage::INDEX32) || usage.contains(HgiBufferUsage::INDEX16) {
        GL_ELEMENT_ARRAY_BUFFER
    } else if usage.contains(HgiBufferUsage::UNIFORM) {
        GL_UNIFORM_BUFFER
    } else if usage.contains(HgiBufferUsage::STORAGE) {
        GL_SHADER_STORAGE_BUFFER
    } else {
        GL_ARRAY_BUFFER
    }
}

/// Converts HGI buffer usage flags to OpenGL usage hint.
pub fn hgi_buffer_usage_to_gl_usage(usage: HgiBufferUsage) -> GLenum {
    if usage.contains(HgiBufferUsage::UPLOAD) {
        GL_DYNAMIC_COPY
    } else {
        GL_STATIC_DRAW
    }
}

/// Converts HGI texture type to OpenGL texture target.
pub fn hgi_texture_type_to_gl_target(texture_type: HgiTextureType) -> GLenum {
    match texture_type {
        HgiTextureType::Texture1D => 0x0DE0, // GL_TEXTURE_1D
        HgiTextureType::Texture2D => GL_TEXTURE_2D,
        HgiTextureType::Texture3D => GL_TEXTURE_3D,
        HgiTextureType::Texture1DArray => 0x8C18, // GL_TEXTURE_1D_ARRAY
        HgiTextureType::Texture2DArray => GL_TEXTURE_2D_ARRAY,
        HgiTextureType::Cubemap => GL_TEXTURE_CUBE_MAP,
    }
}

/// Converts HGI format to OpenGL internal format.
///
/// For depth textures, use `hgi_format_to_gl_format_desc` which accepts
/// `HgiTextureUsage` and handles the `GL_DEPTH_COMPONENT32F` override.
pub fn hgi_format_to_gl_internal_format(format: HgiFormat) -> GLenum {
    hgi_format_to_gl_format_desc_base(format).internal_format
}

/// Depth-aware variant: returns `GL_DEPTH_COMPONENT32F` when `is_depth` is true and
/// `format` is `Float32`; otherwise delegates to `hgi_format_to_gl_internal_format`.
pub fn hgi_format_to_gl_internal_format_with_usage(format: HgiFormat, is_depth: bool) -> GLenum {
    let usage = if is_depth {
        HgiTextureUsage::DEPTH_TARGET
    } else {
        HgiTextureUsage::COLOR_TARGET
    };
    hgi_format_to_gl_format_desc(format, usage).internal_format
}

/// Converts HGI format to OpenGL base pixel format.
///
/// Returns the format component for `glTexImage2D` / `glTexSubImage2D`.
/// For integer types this is `GL_RED_INTEGER` / `GL_RG_INTEGER` etc.
/// For depth/stencil use `hgi_format_to_gl_format_desc` instead.
pub fn hgi_format_to_gl_pixel_format(format: HgiFormat) -> GLenum {
    hgi_format_to_gl_format_desc_base(format).format
}

/// Depth-aware variant: returns `GL_DEPTH_COMPONENT` when `is_depth` is true and
/// `format` is `Float32`; otherwise delegates to `hgi_format_to_gl_pixel_format`.
pub fn hgi_format_to_gl_pixel_format_with_usage(format: HgiFormat, is_depth: bool) -> GLenum {
    let usage = if is_depth {
        HgiTextureUsage::DEPTH_TARGET
    } else {
        HgiTextureUsage::COLOR_TARGET
    };
    hgi_format_to_gl_format_desc(format, usage).format
}

/// Converts HGI format to OpenGL pixel type.
///
/// Returns the `type` parameter for `glTexImage2D` / `glTexSubImage2D`.
pub fn hgi_format_to_gl_pixel_type(format: HgiFormat) -> GLenum {
    hgi_format_to_gl_format_desc_base(format).ty
}

/// Get byte size per pixel for a given HGI format.
pub fn hgi_format_byte_size(format: HgiFormat) -> usize {
    match format {
        HgiFormat::UNorm8 | HgiFormat::SNorm8 => 1,
        HgiFormat::Float16 | HgiFormat::Int16 | HgiFormat::UInt16 => 2,
        HgiFormat::Float32 | HgiFormat::Int32 | HgiFormat::PackedInt1010102 => 4,

        HgiFormat::UNorm8Vec2 | HgiFormat::SNorm8Vec2 => 2,
        HgiFormat::Float16Vec2 | HgiFormat::Int16Vec2 | HgiFormat::UInt16Vec2 => 4,
        HgiFormat::Float32Vec2 | HgiFormat::Int32Vec2 => 8,

        HgiFormat::Float16Vec3 | HgiFormat::Int16Vec3 | HgiFormat::UInt16Vec3 => 6,
        HgiFormat::Float32Vec3 | HgiFormat::Int32Vec3 => 12,

        HgiFormat::UNorm8Vec4 | HgiFormat::SNorm8Vec4 | HgiFormat::UNorm8Vec4srgb => 4,
        HgiFormat::Float16Vec4 | HgiFormat::Int16Vec4 | HgiFormat::UInt16Vec4 => 8,
        HgiFormat::Float32Vec4 | HgiFormat::Int32Vec4 => 16,

        // Compressed: 8 or 16 bytes per 4x4 block — return block size
        HgiFormat::BC1UNorm8Vec4 | HgiFormat::BC3UNorm8Vec4 => 16,
        HgiFormat::BC6FloatVec3 | HgiFormat::BC6UFloatVec3 => 16,
        HgiFormat::BC7UNorm8Vec4 | HgiFormat::BC7UNorm8Vec4srgb => 16,

        // Depth+stencil: 4 (depth) + 4 (stencil word) = 8 bytes per pixel
        HgiFormat::Float32UInt8 => 8,

        // 16-bit depth-only
        HgiFormat::PackedD16Unorm => 2,

        HgiFormat::Invalid => 4,
    }
}

/// Converts HGI sampler filter modes to OpenGL minification filter.
pub fn hgi_sampler_filter_to_gl_min_filter(filter: HgiSamplerFilter, mip: HgiMipFilter) -> GLenum {
    match (filter, mip) {
        (HgiSamplerFilter::Nearest, HgiMipFilter::NotMipmapped) => GL_NEAREST,
        (HgiSamplerFilter::Linear, HgiMipFilter::NotMipmapped) => GL_LINEAR,
        (HgiSamplerFilter::Nearest, HgiMipFilter::Nearest) => 0x2700, // GL_NEAREST_MIPMAP_NEAREST
        (HgiSamplerFilter::Linear, HgiMipFilter::Nearest) => 0x2701,  // GL_LINEAR_MIPMAP_NEAREST
        (HgiSamplerFilter::Nearest, HgiMipFilter::Linear) => 0x2702,  // GL_NEAREST_MIPMAP_LINEAR
        (HgiSamplerFilter::Linear, HgiMipFilter::Linear) => GL_LINEAR_MIPMAP_LINEAR,
    }
}

/// Converts HGI sampler filter to OpenGL magnification filter.
pub fn hgi_sampler_filter_to_gl_mag_filter(filter: HgiSamplerFilter) -> GLenum {
    match filter {
        HgiSamplerFilter::Nearest => GL_NEAREST,
        HgiSamplerFilter::Linear => GL_LINEAR,
    }
}

/// Converts HGI sampler address mode to OpenGL texture wrap mode.
pub fn hgi_address_mode_to_gl_wrap(mode: HgiSamplerAddressMode) -> GLenum {
    match mode {
        HgiSamplerAddressMode::Repeat => GL_REPEAT,
        HgiSamplerAddressMode::MirrorRepeat => GL_MIRRORED_REPEAT,
        HgiSamplerAddressMode::ClampToEdge => GL_CLAMP_TO_EDGE,
        HgiSamplerAddressMode::MirrorClampToEdge => 0x8743, // GL_MIRROR_CLAMP_TO_EDGE
        HgiSamplerAddressMode::ClampToBorderColor => GL_CLAMP_TO_BORDER,
    }
}

/// Converts HGI comparison function to OpenGL comparison function.
pub fn hgi_compare_func_to_gl(func: HgiCompareFunction) -> GLenum {
    match func {
        HgiCompareFunction::Never => 0x0200,    // GL_NEVER
        HgiCompareFunction::Less => 0x0201,     // GL_LESS
        HgiCompareFunction::Equal => 0x0202,    // GL_EQUAL
        HgiCompareFunction::LEqual => 0x0203,   // GL_LEQUAL
        HgiCompareFunction::Greater => 0x0204,  // GL_GREATER
        HgiCompareFunction::NotEqual => 0x0205, // GL_NOTEQUAL
        HgiCompareFunction::GEqual => 0x0206,   // GL_GEQUAL
        HgiCompareFunction::Always => 0x0207,   // GL_ALWAYS
    }
}

/// Converts HGI blend operation to OpenGL blend equation.
pub fn hgi_blend_op_to_gl(op: HgiBlendOp) -> GLenum {
    match op {
        HgiBlendOp::Add => 0x8006,             // GL_FUNC_ADD
        HgiBlendOp::Subtract => 0x800A,        // GL_FUNC_SUBTRACT
        HgiBlendOp::ReverseSubtract => 0x800B, // GL_FUNC_REVERSE_SUBTRACT
        HgiBlendOp::Min => 0x8007,             // GL_MIN
        HgiBlendOp::Max => 0x8008,             // GL_MAX
    }
}

/// Converts HGI blend factor to OpenGL blend factor.
pub fn hgi_blend_factor_to_gl(factor: HgiBlendFactor) -> GLenum {
    match factor {
        HgiBlendFactor::Zero => 0x0000,     // GL_ZERO
        HgiBlendFactor::One => 0x0001,      // GL_ONE
        HgiBlendFactor::SrcColor => 0x0300, // GL_SRC_COLOR
        HgiBlendFactor::OneMinusSrcColor => 0x0301,
        HgiBlendFactor::DstColor => 0x0306,
        HgiBlendFactor::OneMinusDstColor => 0x0307,
        HgiBlendFactor::SrcAlpha => 0x0302,
        HgiBlendFactor::OneMinusSrcAlpha => 0x0303,
        HgiBlendFactor::DstAlpha => 0x0304,
        HgiBlendFactor::OneMinusDstAlpha => 0x0305,
        HgiBlendFactor::ConstantColor => 0x8001,
        HgiBlendFactor::OneMinusConstantColor => 0x8002,
        HgiBlendFactor::ConstantAlpha => 0x8003,
        HgiBlendFactor::OneMinusConstantAlpha => 0x8004,
        HgiBlendFactor::SrcAlphaSaturate => 0x0308,
        HgiBlendFactor::Src1Color => 0x88F9,
        HgiBlendFactor::OneMinusSrc1Color => 0x88FA,
        HgiBlendFactor::Src1Alpha => 0x8589,
        HgiBlendFactor::OneMinusSrc1Alpha => 0x88FB,
    }
}

/// Converts HGI cull mode to OpenGL cull face mode.
///
/// Returns `None` when culling is disabled.
pub fn hgi_cull_mode_to_gl(mode: HgiCullMode) -> Option<GLenum> {
    match mode {
        HgiCullMode::None => None,
        HgiCullMode::Front => Some(0x0404),        // GL_FRONT
        HgiCullMode::Back => Some(0x0405),         // GL_BACK
        HgiCullMode::FrontAndBack => Some(0x0408), // GL_FRONT_AND_BACK
    }
}

/// Converts HGI winding order to OpenGL front face orientation.
pub fn hgi_winding_to_gl(winding: HgiWinding) -> GLenum {
    match winding {
        HgiWinding::Clockwise => 0x0900,        // GL_CW
        HgiWinding::CounterClockwise => 0x0901, // GL_CCW
    }
}

/// Converts HGI polygon mode to OpenGL polygon mode.
pub fn hgi_polygon_mode_to_gl(mode: HgiPolygonMode) -> GLenum {
    match mode {
        HgiPolygonMode::Fill => 0x1B02,  // GL_FILL
        HgiPolygonMode::Line => 0x1B01,  // GL_LINE
        HgiPolygonMode::Point => 0x1B00, // GL_POINT
    }
}

/// Converts HGI border color to GL border color RGBA array.
pub fn hgi_border_color_to_gl(color: HgiBorderColor) -> [f32; 4] {
    match color {
        HgiBorderColor::TransparentBlack => [0.0, 0.0, 0.0, 0.0],
        HgiBorderColor::OpaqueBlack => [0.0, 0.0, 0.0, 1.0],
        HgiBorderColor::OpaqueWhite => [1.0, 1.0, 1.0, 1.0],
    }
}

/// Converts HGI stencil operation to OpenGL stencil operation.
pub fn hgi_stencil_op_to_gl(op: HgiStencilOp) -> GLenum {
    match op {
        HgiStencilOp::Keep => 0x1E00,           // GL_KEEP
        HgiStencilOp::Zero => 0x0000,           // GL_ZERO
        HgiStencilOp::Replace => 0x1E01,        // GL_REPLACE
        HgiStencilOp::IncrementClamp => 0x1E02, // GL_INCR
        HgiStencilOp::DecrementClamp => 0x1E03, // GL_DECR
        HgiStencilOp::Invert => 0x150A,         // GL_INVERT
        HgiStencilOp::IncrementWrap => 0x8507,  // GL_INCR_WRAP
        HgiStencilOp::DecrementWrap => 0x8508,  // GL_DECR_WRAP
    }
}

/// Converts HGI component swizzle to OpenGL swizzle value.
pub fn hgi_component_swizzle_to_gl(swizzle: HgiComponentSwizzle) -> GLenum {
    match swizzle {
        HgiComponentSwizzle::Zero => 0x0000, // GL_ZERO
        HgiComponentSwizzle::One => 0x0001,  // GL_ONE
        HgiComponentSwizzle::R => 0x1903,    // GL_RED
        HgiComponentSwizzle::G => 0x1904,    // GL_GREEN
        HgiComponentSwizzle::B => 0x1905,    // GL_BLUE
        HgiComponentSwizzle::A => 0x1906,    // GL_ALPHA
    }
}

/// Converts HGI primitive type to OpenGL primitive type.
pub fn hgi_primitive_type_to_gl(pt: HgiPrimitiveType) -> GLenum {
    match pt {
        HgiPrimitiveType::PointList => 0x0000,             // GL_POINTS
        HgiPrimitiveType::LineList => 0x0001,              // GL_LINES
        HgiPrimitiveType::LineStrip => 0x000A,             // GL_LINES_ADJACENCY (matches C++)
        HgiPrimitiveType::TriangleList => 0x0004,          // GL_TRIANGLES
        HgiPrimitiveType::PatchList => 0x000E,             // GL_PATCHES
        HgiPrimitiveType::LineListWithAdjacency => 0x000A, // GL_LINES_ADJACENCY
    }
}

/// Converts HGI shader stage flags to a list of OpenGL shader type constants.
pub fn hgi_shader_stage_to_gl(stages: HgiShaderStage) -> Vec<GLenum> {
    let mut result = Vec::new();
    if stages.contains(HgiShaderStage::VERTEX) {
        result.push(0x8B31);
    } // GL_VERTEX_SHADER
    if stages.contains(HgiShaderStage::FRAGMENT) {
        result.push(0x8B30);
    } // GL_FRAGMENT_SHADER
    if stages.contains(HgiShaderStage::COMPUTE) {
        result.push(0x91B9);
    } // GL_COMPUTE_SHADER
    if stages.contains(HgiShaderStage::TESSELLATION_CONTROL) {
        result.push(0x8E88);
    } // GL_TESS_CONTROL_SHADER
    if stages.contains(HgiShaderStage::TESSELLATION_EVAL) {
        result.push(0x8E87);
    } // GL_TESS_EVALUATION_SHADER
    if stages.contains(HgiShaderStage::GEOMETRY) {
        result.push(0x8DD9);
    } // GL_GEOMETRY_SHADER
    result
}

/// Returns the GLSL image layout format qualifier string for a given format.
///
/// Matches `_imageLayoutFormatTable` from C++ `conversions.cpp`.
/// Formats with no valid GLSL image qualifier (e.g. RGB, compressed) return `"rgba16f"` as
/// default, matching C++ behaviour.
pub fn hgi_image_layout_format_qualifier(format: HgiFormat) -> &'static str {
    match format {
        HgiFormat::UNorm8 => "r8",
        HgiFormat::UNorm8Vec2 => "rg8",
        HgiFormat::UNorm8Vec4 => "rgba8",
        HgiFormat::SNorm8 => "r8_snorm",
        HgiFormat::SNorm8Vec2 => "rg8_snorm",
        HgiFormat::SNorm8Vec4 => "rgba8_snorm",
        HgiFormat::Float16 => "r16f",
        HgiFormat::Float16Vec2 => "rg16f",
        // Float16Vec3: no valid qualifier in GLSL, fallback
        HgiFormat::Float16Vec4 => "rgba16f",
        HgiFormat::Float32 => "r32f",
        HgiFormat::Float32Vec2 => "rg32f",
        // Float32Vec3: no valid qualifier
        HgiFormat::Float32Vec4 => "rgba32f",
        HgiFormat::Int16 => "r16i",
        HgiFormat::Int16Vec2 => "rg16i",
        // Int16Vec3: no valid qualifier
        HgiFormat::Int16Vec4 => "rgba16i",
        HgiFormat::UInt16 => "r16ui",
        HgiFormat::UInt16Vec2 => "rg16ui",
        // UInt16Vec3: no valid qualifier
        HgiFormat::UInt16Vec4 => "rgba16ui",
        HgiFormat::Int32 => "r32i",
        HgiFormat::Int32Vec2 => "rg32i",
        // Int32Vec3: no valid qualifier
        HgiFormat::Int32Vec4 => "rgba32i",
        // Compressed, sRGB, depth-stencil, packed: no GLSL image layout qualifier
        _ => "rgba16f",
    }
}

/// Returns `true` if the GL type for this format is an integer (non-float) type.
///
/// Integer formats require `glVertexAttribIPointer` instead of `glVertexAttribPointer`.
/// Mirrors C++ `HgiGLConversions::IsVertexAttribIntegerFormat()`.
pub fn is_vertex_attrib_integer_format(format: HgiFormat) -> bool {
    let ty = hgi_format_to_gl_format_desc_base(format).ty;
    matches!(
        ty,
        0x1400 | // GL_BYTE
        0x1401 | // GL_UNSIGNED_BYTE
        GL_SHORT |
        GL_UNSIGNED_SHORT |
        0x1404 | // GL_INT
        0x1405 // GL_UNSIGNED_INT
    )
}

// ---- Tests -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_conversions() {
        assert_eq!(
            hgi_buffer_usage_to_gl_target(HgiBufferUsage::VERTEX),
            GL_ARRAY_BUFFER
        );
        assert_eq!(
            hgi_buffer_usage_to_gl_target(HgiBufferUsage::INDEX32),
            GL_ELEMENT_ARRAY_BUFFER
        );
    }

    #[test]
    fn test_buffer_usage() {
        assert_eq!(
            hgi_buffer_usage_to_gl_usage(HgiBufferUsage::UPLOAD),
            GL_DYNAMIC_COPY
        );
        assert_eq!(
            hgi_buffer_usage_to_gl_usage(HgiBufferUsage::VERTEX),
            GL_STATIC_DRAW
        );
    }

    #[test]
    fn test_texture_conversions() {
        assert_eq!(
            hgi_texture_type_to_gl_target(HgiTextureType::Texture2D),
            GL_TEXTURE_2D
        );
        assert_eq!(
            hgi_texture_type_to_gl_target(HgiTextureType::Texture3D),
            GL_TEXTURE_3D
        );
    }

    #[test]
    fn test_sampler_conversions() {
        assert_eq!(
            hgi_sampler_filter_to_gl_mag_filter(HgiSamplerFilter::Linear),
            GL_LINEAR
        );
        assert_eq!(
            hgi_address_mode_to_gl_wrap(HgiSamplerAddressMode::Repeat),
            GL_REPEAT
        );
    }

    #[test]
    fn test_stencil_ops() {
        assert_eq!(hgi_stencil_op_to_gl(HgiStencilOp::Keep), 0x1E00);
        assert_eq!(hgi_stencil_op_to_gl(HgiStencilOp::Replace), 0x1E01);
        assert_eq!(hgi_stencil_op_to_gl(HgiStencilOp::IncrementWrap), 0x8507);
    }

    #[test]
    fn test_component_swizzle() {
        assert_eq!(
            hgi_component_swizzle_to_gl(HgiComponentSwizzle::Zero),
            0x0000
        );
        assert_eq!(hgi_component_swizzle_to_gl(HgiComponentSwizzle::R), 0x1903);
        assert_eq!(hgi_component_swizzle_to_gl(HgiComponentSwizzle::A), 0x1906);
    }

    #[test]
    fn test_primitive_type() {
        assert_eq!(
            hgi_primitive_type_to_gl(HgiPrimitiveType::PointList),
            0x0000
        );
        assert_eq!(
            hgi_primitive_type_to_gl(HgiPrimitiveType::TriangleList),
            0x0004
        );
        assert_eq!(
            hgi_primitive_type_to_gl(HgiPrimitiveType::PatchList),
            0x000E
        );
    }

    #[test]
    fn test_shader_stages() {
        let stages = HgiShaderStage::VERTEX | HgiShaderStage::FRAGMENT;
        let gl_stages = hgi_shader_stage_to_gl(stages);
        assert_eq!(gl_stages.len(), 2);
        assert!(gl_stages.contains(&0x8B31)); // GL_VERTEX_SHADER
        assert!(gl_stages.contains(&0x8B30)); // GL_FRAGMENT_SHADER
    }

    #[test]
    fn test_image_layout_format() {
        assert_eq!(
            hgi_image_layout_format_qualifier(HgiFormat::UNorm8Vec4),
            "rgba8"
        );
        assert_eq!(
            hgi_image_layout_format_qualifier(HgiFormat::Float32),
            "r32f"
        );
        assert_eq!(
            hgi_image_layout_format_qualifier(HgiFormat::Float16Vec4),
            "rgba16f"
        );
        // formats without GLSL image qualifier fall back to rgba16f
        assert_eq!(
            hgi_image_layout_format_qualifier(HgiFormat::Float16Vec3),
            "rgba16f"
        );
        assert_eq!(
            hgi_image_layout_format_qualifier(HgiFormat::UNorm8Vec4srgb),
            "rgba16f"
        );
        assert_eq!(
            hgi_image_layout_format_qualifier(HgiFormat::BC7UNorm8Vec4),
            "rgba16f"
        );
    }

    #[test]
    fn test_integer_format_detection() {
        // Float types are NOT integer vertex attrib formats
        assert!(!is_vertex_attrib_integer_format(HgiFormat::Float32));
        assert!(!is_vertex_attrib_integer_format(HgiFormat::Float16Vec4));
        assert!(!is_vertex_attrib_integer_format(HgiFormat::Float32Vec3));
        // Integer types ARE integer vertex attrib formats
        assert!(is_vertex_attrib_integer_format(HgiFormat::Int32));
        assert!(is_vertex_attrib_integer_format(HgiFormat::Int16));
        assert!(is_vertex_attrib_integer_format(HgiFormat::UInt16Vec4));
        // UNorm/SNorm use BYTE/UBYTE — also integer vertex attrib
        assert!(is_vertex_attrib_integer_format(HgiFormat::UNorm8));
        assert!(is_vertex_attrib_integer_format(HgiFormat::SNorm8Vec4));
    }

    // BUG 1: depth texture format override
    #[test]
    fn test_depth_texture_override() {
        // Without depth usage: Float32 -> R32F
        let no_depth =
            hgi_format_to_gl_format_desc(HgiFormat::Float32, HgiTextureUsage::SHADER_READ);
        assert_eq!(no_depth.internal_format, 0x8236); // GL_R32F
        assert_eq!(no_depth.format, 0x1903); // GL_RED

        // With depth usage: Float32 -> DEPTH_COMPONENT32F
        let depth = hgi_format_to_gl_format_desc(HgiFormat::Float32, HgiTextureUsage::DEPTH_TARGET);
        assert_eq!(depth.internal_format, GL_DEPTH_COMPONENT32F);
        assert_eq!(depth.format, GL_DEPTH_COMPONENT);

        // Non-Float32 with depth usage is NOT overridden
        let depth_rgba =
            hgi_format_to_gl_format_desc(HgiFormat::Float32Vec4, HgiTextureUsage::DEPTH_TARGET);
        assert_eq!(depth_rgba.internal_format, GL_RGBA32F);
    }

    // BUG 2: previously missing formats
    #[test]
    fn test_previously_missing_formats() {
        // Int16
        assert_eq!(hgi_format_to_gl_internal_format(HgiFormat::Int16), GL_R16I);
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::Int16Vec2),
            GL_RG16I
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::Int16Vec3),
            GL_RGB16I
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::Int16Vec4),
            GL_RGBA16I
        );

        // UInt16
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::UInt16),
            GL_R16UI
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::UInt16Vec2),
            GL_RG16UI
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::UInt16Vec3),
            GL_RGB16UI
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::UInt16Vec4),
            GL_RGBA16UI
        );

        // Float16Vec3
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::Float16Vec3),
            GL_RGB16F
        );

        // sRGB
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::UNorm8Vec4srgb),
            GL_SRGB8_ALPHA8
        );

        // BPTC (BC6/BC7)
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::BC6FloatVec3),
            GL_COMPRESSED_RGB_BPTC_SIGNED_FLOAT
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::BC6UFloatVec3),
            GL_COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::BC7UNorm8Vec4),
            GL_COMPRESSED_RGBA_BPTC_UNORM
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::BC7UNorm8Vec4srgb),
            GL_COMPRESSED_SRGB_ALPHA_BPTC_UNORM
        );

        // S3TC (BC1/BC3)
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::BC1UNorm8Vec4),
            GL_COMPRESSED_RGBA_S3TC_DXT1_EXT
        );
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::BC3UNorm8Vec4),
            GL_COMPRESSED_RGBA_S3TC_DXT5_EXT
        );

        // Float32UInt8 (depth+stencil)
        assert_eq!(
            hgi_format_to_gl_internal_format(HgiFormat::Float32UInt8),
            GL_DEPTH32F_STENCIL8
        );

        // PackedInt1010102
        assert_eq!(
            hgi_format_to_gl_format_desc_base(HgiFormat::PackedInt1010102).ty,
            GL_INT_2_10_10_10_REV
        );

        // pixel formats for integer types use integer base formats
        assert_eq!(
            hgi_format_to_gl_pixel_format(HgiFormat::Int16),
            GL_RED_INTEGER
        );
        assert_eq!(
            hgi_format_to_gl_pixel_format(HgiFormat::UInt16Vec4),
            GL_RGBA_INTEGER
        );
        assert_eq!(
            hgi_format_to_gl_pixel_format(HgiFormat::Float32UInt8),
            GL_DEPTH_STENCIL
        );

        // pixel types
        assert_eq!(hgi_format_to_gl_pixel_type(HgiFormat::Int16), GL_SHORT);
        assert_eq!(
            hgi_format_to_gl_pixel_type(HgiFormat::UInt16Vec2),
            GL_UNSIGNED_SHORT
        );
    }
}
