//! HGI to Metal type conversions. Port of pxr/imaging/hgiMetal/conversions
//!
//! Provides mapping from HGI abstract enums to Metal equivalents.
//! On non-Apple platforms these return placeholder u32 values matching
//! the Metal enum integer values for testing/compilation purposes.

use usd_hgi::*;

/// Metal pixel format values (subset matching MTLPixelFormat).
/// Used as return type on non-Apple platforms.
pub type MtlPixelFormat = u32;
pub type MtlVertexFormat = u32;
pub type MtlCullMode = u32;
pub type MtlTriangleFillMode = u32;
pub type MtlBlendFactor = u32;
pub type MtlBlendOperation = u32;
pub type MtlWinding = u32;
pub type MtlLoadAction = u32;
pub type MtlStoreAction = u32;
pub type MtlCompareFunction = u32;
pub type MtlStencilOperation = u32;
pub type MtlTextureType = u32;
pub type MtlSamplerAddressMode = u32;
pub type MtlSamplerMinMagFilter = u32;
pub type MtlSamplerMipFilter = u32;
pub type MtlSamplerBorderColor = u32;
pub type MtlTextureSwizzle = u32;
pub type MtlPrimitiveTopologyClass = u32;
pub type MtlPrimitiveType = u32;
pub type MtlColorWriteMask = u32;

/// Converts from Hgi types to Metal types.
/// Mirrors C++ HgiMetalConversions.
pub struct HgiMetalConversions;

impl HgiMetalConversions {
    /// Convert HgiFormat + usage to MTLPixelFormat.
    /// When usage includes DEPTH_TARGET, returns depth-specific pixel formats.
    pub fn get_pixel_format(format: HgiFormat, usage: HgiTextureUsage) -> MtlPixelFormat {
        // C++: depth target textures get depth-specific pixel formats
        if usage.contains(HgiTextureUsage::DEPTH_TARGET) {
            return match format {
                HgiFormat::Float32UInt8 => 260, // MTLPixelFormatDepth32Float_Stencil8
                _ => 252,                       // MTLPixelFormatDepth32Float
            };
        }

        match format {
            HgiFormat::UNorm8 => 10,             // MTLPixelFormatR8Unorm
            HgiFormat::UNorm8Vec2 => 30,         // MTLPixelFormatRG8Unorm
            HgiFormat::UNorm8Vec4 => 70,         // MTLPixelFormatRGBA8Unorm
            HgiFormat::SNorm8 => 12,             // MTLPixelFormatR8Snorm
            HgiFormat::SNorm8Vec2 => 32,         // MTLPixelFormatRG8Snorm
            HgiFormat::SNorm8Vec4 => 72,         // MTLPixelFormatRGBA8Snorm
            HgiFormat::Float16 => 25,            // MTLPixelFormatR16Float
            HgiFormat::Float16Vec2 => 55,        // MTLPixelFormatRG16Float
            HgiFormat::Float16Vec3 => 55,        // No direct 3-comp, use RG16Float
            HgiFormat::Float16Vec4 => 115,       // MTLPixelFormatRGBA16Float
            HgiFormat::Float32 => 28,            // MTLPixelFormatR32Float
            HgiFormat::Float32Vec2 => 58,        // MTLPixelFormatRG32Float
            HgiFormat::Float32Vec3 => 58,        // No direct 3-comp
            HgiFormat::Float32Vec4 => 125,       // MTLPixelFormatRGBA32Float
            HgiFormat::Int16 => 22,              // MTLPixelFormatR16Sint
            HgiFormat::Int16Vec2 => 52,          // MTLPixelFormatRG16Sint
            HgiFormat::Int16Vec3 => 52,          // No direct 3-comp
            HgiFormat::Int16Vec4 => 112,         // MTLPixelFormatRGBA16Sint
            HgiFormat::UInt16 => 23,             // MTLPixelFormatR16Uint
            HgiFormat::UInt16Vec2 => 53,         // MTLPixelFormatRG16Uint
            HgiFormat::UInt16Vec3 => 53,         // No direct 3-comp
            HgiFormat::UInt16Vec4 => 113,        // MTLPixelFormatRGBA16Uint
            HgiFormat::Int32 => 24,              // MTLPixelFormatR32Sint
            HgiFormat::Int32Vec2 => 54,          // MTLPixelFormatRG32Sint
            HgiFormat::Int32Vec3 => 54,          // No direct 3-comp
            HgiFormat::Int32Vec4 => 124,         // MTLPixelFormatRGBA32Sint
            HgiFormat::UNorm8Vec4srgb => 71,     // MTLPixelFormatRGBA8Unorm_sRGB
            HgiFormat::BC6FloatVec3 => 241,      // MTLPixelFormatBC6H_RGBFloat
            HgiFormat::BC6UFloatVec3 => 242,     // MTLPixelFormatBC6H_RGBUfloat
            HgiFormat::BC7UNorm8Vec4 => 254,     // MTLPixelFormatBC7_RGBAUnorm
            HgiFormat::BC7UNorm8Vec4srgb => 255, // MTLPixelFormatBC7_RGBAUnorm_sRGB
            HgiFormat::BC1UNorm8Vec4 => 130,     // MTLPixelFormatBC1_RGBA
            HgiFormat::BC3UNorm8Vec4 => 134,     // MTLPixelFormatBC3_RGBA
            HgiFormat::Float32UInt8 => 260,      // MTLPixelFormatDepth32Float_Stencil8
            HgiFormat::PackedInt1010102 => 259,  // MTLPixelFormatRGB10A2Uint
            HgiFormat::PackedD16Unorm => 250,    // MTLPixelFormatDepth16Unorm
            HgiFormat::Invalid => 0,             // MTLPixelFormatInvalid
        }
    }

    /// Convert HgiFormat to MTLVertexFormat.
    pub fn get_vertex_format(format: HgiFormat) -> MtlVertexFormat {
        match format {
            HgiFormat::UNorm8 => 45,           // MTLVertexFormatUCharNormalized
            HgiFormat::UNorm8Vec2 => 46,       // MTLVertexFormatUChar2Normalized
            HgiFormat::UNorm8Vec4 => 48,       // MTLVertexFormatUChar4Normalized
            HgiFormat::SNorm8 => 39,           // MTLVertexFormatCharNormalized
            HgiFormat::SNorm8Vec2 => 40,       // MTLVertexFormatChar2Normalized
            HgiFormat::SNorm8Vec4 => 42,       // MTLVertexFormatChar4Normalized
            HgiFormat::Float16 => 53,          // MTLVertexFormatHalf
            HgiFormat::Float16Vec2 => 25,      // MTLVertexFormatHalf2
            HgiFormat::Float16Vec3 => 26,      // MTLVertexFormatHalf3
            HgiFormat::Float16Vec4 => 27,      // MTLVertexFormatHalf4
            HgiFormat::Float32 => 28,          // MTLVertexFormatFloat
            HgiFormat::Float32Vec2 => 29,      // MTLVertexFormatFloat2
            HgiFormat::Float32Vec3 => 30,      // MTLVertexFormatFloat3
            HgiFormat::Float32Vec4 => 31,      // MTLVertexFormatFloat4
            HgiFormat::Int16 => 51,            // MTLVertexFormatShort
            HgiFormat::Int16Vec2 => 16,        // MTLVertexFormatShort2
            HgiFormat::Int16Vec3 => 17,        // MTLVertexFormatShort3
            HgiFormat::Int16Vec4 => 18,        // MTLVertexFormatShort4
            HgiFormat::UInt16 => 52,           // MTLVertexFormatUShort
            HgiFormat::UInt16Vec2 => 19,       // MTLVertexFormatUShort2
            HgiFormat::UInt16Vec3 => 20,       // MTLVertexFormatUShort3
            HgiFormat::UInt16Vec4 => 21,       // MTLVertexFormatUShort4
            HgiFormat::Int32 => 32,            // MTLVertexFormatInt
            HgiFormat::Int32Vec2 => 33,        // MTLVertexFormatInt2
            HgiFormat::Int32Vec3 => 34,        // MTLVertexFormatInt3
            HgiFormat::Int32Vec4 => 35,        // MTLVertexFormatInt4
            HgiFormat::PackedInt1010102 => 36, // MTLVertexFormatInt1010102Normalized
            _ => 0,                            // MTLVertexFormatInvalid
        }
    }

    /// Convert HgiCullMode to MTLCullMode.
    pub fn get_cull_mode(mode: HgiCullMode) -> MtlCullMode {
        match mode {
            HgiCullMode::None => 0,         // MTLCullModeNone
            HgiCullMode::Front => 1,        // MTLCullModeFront
            HgiCullMode::Back => 2,         // MTLCullModeBack
            HgiCullMode::FrontAndBack => 0, // No direct equivalent, use none
        }
    }

    /// Convert HgiPolygonMode to MTLTriangleFillMode.
    pub fn get_polygon_mode(mode: HgiPolygonMode) -> MtlTriangleFillMode {
        match mode {
            HgiPolygonMode::Fill => 0,  // MTLTriangleFillModeFill
            HgiPolygonMode::Line => 1,  // MTLTriangleFillModeLines
            HgiPolygonMode::Point => 0, // No direct equivalent
        }
    }

    /// Convert HgiBlendFactor to MTLBlendFactor.
    /// Matches C++ table: ConstantColor/Alpha variants map to Zero (unsupported on Metal).
    pub fn get_blend_factor(factor: HgiBlendFactor) -> MtlBlendFactor {
        match factor {
            HgiBlendFactor::Zero => 0,
            HgiBlendFactor::One => 1,
            HgiBlendFactor::SrcColor => 2,
            HgiBlendFactor::OneMinusSrcColor => 3,
            HgiBlendFactor::DstColor => 6,
            HgiBlendFactor::OneMinusDstColor => 7,
            HgiBlendFactor::SrcAlpha => 4,
            HgiBlendFactor::OneMinusSrcAlpha => 5,
            HgiBlendFactor::DstAlpha => 8,
            HgiBlendFactor::OneMinusDstAlpha => 9,
            // C++ maps these to Zero (unsupported on Metal)
            HgiBlendFactor::ConstantColor => 0,
            HgiBlendFactor::OneMinusConstantColor => 0,
            HgiBlendFactor::ConstantAlpha => 0,
            HgiBlendFactor::OneMinusConstantAlpha => 0,
            HgiBlendFactor::SrcAlphaSaturate => 10,
            HgiBlendFactor::Src1Color => 15,
            HgiBlendFactor::OneMinusSrc1Color => 16,
            // C++ maps Src1Alpha -> MTLBlendFactorSourceAlpha (4), not Source1Alpha
            HgiBlendFactor::Src1Alpha => 4,
            HgiBlendFactor::OneMinusSrc1Alpha => 18,
        }
    }

    /// Convert HgiBlendOp to MTLBlendOperation.
    pub fn get_blend_equation(op: HgiBlendOp) -> MtlBlendOperation {
        match op {
            HgiBlendOp::Add => 0,             // MTLBlendOperationAdd
            HgiBlendOp::Subtract => 1,        // MTLBlendOperationSubtract
            HgiBlendOp::ReverseSubtract => 2, // MTLBlendOperationReverseSubtract
            HgiBlendOp::Min => 3,             // MTLBlendOperationMin
            HgiBlendOp::Max => 4,             // MTLBlendOperationMax
        }
    }

    /// Convert HgiWinding to MTLWinding.
    /// NOTE: Winding is intentionally inverted to emulate OpenGL coordinate space on Metal.
    /// C++ comment: "Winding order is inverted because our viewport is inverted."
    pub fn get_winding(winding: HgiWinding) -> MtlWinding {
        match winding {
            HgiWinding::Clockwise => 1, // MTLWindingCounterClockwise (inverted!)
            HgiWinding::CounterClockwise => 0, // MTLWindingClockwise (inverted!)
        }
    }

    /// Convert HgiAttachmentLoadOp to MTLLoadAction.
    pub fn get_attachment_load_op(op: HgiAttachmentLoadOp) -> MtlLoadAction {
        match op {
            HgiAttachmentLoadOp::DontCare => 0, // MTLLoadActionDontCare
            HgiAttachmentLoadOp::Clear => 2,    // MTLLoadActionClear
            HgiAttachmentLoadOp::Load => 1,     // MTLLoadActionLoad
        }
    }

    /// Convert HgiAttachmentStoreOp to MTLStoreAction.
    pub fn get_attachment_store_op(op: HgiAttachmentStoreOp) -> MtlStoreAction {
        match op {
            HgiAttachmentStoreOp::DontCare => 0, // MTLStoreActionDontCare
            HgiAttachmentStoreOp::Store => 1,    // MTLStoreActionStore
        }
    }

    /// Convert HgiCompareFunction to MTLCompareFunction.
    pub fn get_compare_function(func: HgiCompareFunction) -> MtlCompareFunction {
        match func {
            HgiCompareFunction::Never => 0,    // MTLCompareFunctionNever
            HgiCompareFunction::Less => 1,     // MTLCompareFunctionLess
            HgiCompareFunction::Equal => 2,    // MTLCompareFunctionEqual
            HgiCompareFunction::LEqual => 3,   // MTLCompareFunctionLessEqual
            HgiCompareFunction::Greater => 4,  // MTLCompareFunctionGreater
            HgiCompareFunction::NotEqual => 5, // MTLCompareFunctionNotEqual
            HgiCompareFunction::GEqual => 6,   // MTLCompareFunctionGreaterEqual
            HgiCompareFunction::Always => 7,   // MTLCompareFunctionAlways
        }
    }

    /// Convert HgiStencilOp to MTLStencilOperation.
    pub fn get_stencil_op(op: HgiStencilOp) -> MtlStencilOperation {
        match op {
            HgiStencilOp::Keep => 0,           // MTLStencilOperationKeep
            HgiStencilOp::Zero => 1,           // MTLStencilOperationZero
            HgiStencilOp::Replace => 2,        // MTLStencilOperationReplace
            HgiStencilOp::IncrementClamp => 3, // MTLStencilOperationIncrementClamp
            HgiStencilOp::DecrementClamp => 4, // MTLStencilOperationDecrementClamp
            HgiStencilOp::Invert => 5,         // MTLStencilOperationInvert
            HgiStencilOp::IncrementWrap => 6,  // MTLStencilOperationIncrementWrap
            HgiStencilOp::DecrementWrap => 7,  // MTLStencilOperationDecrementWrap
        }
    }

    /// Convert HgiTextureType to MTLTextureType.
    pub fn get_texture_type(texture_type: HgiTextureType) -> MtlTextureType {
        match texture_type {
            HgiTextureType::Texture1D => 0,      // MTLTextureType1D
            HgiTextureType::Texture2D => 2,      // MTLTextureType2D
            HgiTextureType::Texture3D => 4,      // MTLTextureType3D
            HgiTextureType::Cubemap => 6,        // MTLTextureTypeCube
            HgiTextureType::Texture1DArray => 1, // MTLTextureType1DArray
            HgiTextureType::Texture2DArray => 3, // MTLTextureType2DArray
        }
    }

    /// Convert HgiSamplerAddressMode to MTLSamplerAddressMode.
    pub fn get_sampler_address_mode(mode: HgiSamplerAddressMode) -> MtlSamplerAddressMode {
        match mode {
            HgiSamplerAddressMode::ClampToEdge => 0, // MTLSamplerAddressModeClampToEdge
            HgiSamplerAddressMode::MirrorClampToEdge => 1, // MTLSamplerAddressModeMirrorClampToEdge
            HgiSamplerAddressMode::Repeat => 2,      // MTLSamplerAddressModeRepeat
            HgiSamplerAddressMode::MirrorRepeat => 3, // MTLSamplerAddressModeMirrorRepeat
            HgiSamplerAddressMode::ClampToBorderColor => 4, // MTLSamplerAddressModeClampToBorderColor
        }
    }

    /// Convert HgiSamplerFilter to MTLSamplerMinMagFilter.
    pub fn get_min_mag_filter(filter: HgiSamplerFilter) -> MtlSamplerMinMagFilter {
        match filter {
            HgiSamplerFilter::Nearest => 0, // MTLSamplerMinMagFilterNearest
            HgiSamplerFilter::Linear => 1,  // MTLSamplerMinMagFilterLinear
        }
    }

    /// Convert HgiMipFilter to MTLSamplerMipFilter.
    pub fn get_mip_filter(filter: HgiMipFilter) -> MtlSamplerMipFilter {
        match filter {
            HgiMipFilter::NotMipmapped => 0, // MTLSamplerMipFilterNotMipmapped
            HgiMipFilter::Nearest => 1,      // MTLSamplerMipFilterNearest
            HgiMipFilter::Linear => 2,       // MTLSamplerMipFilterLinear
        }
    }

    /// Convert HgiBorderColor to MTLSamplerBorderColor.
    pub fn get_border_color(color: HgiBorderColor) -> MtlSamplerBorderColor {
        match color {
            HgiBorderColor::TransparentBlack => 0, // MTLSamplerBorderColorTransparentBlack
            HgiBorderColor::OpaqueBlack => 1,      // MTLSamplerBorderColorOpaqueBlack
            HgiBorderColor::OpaqueWhite => 2,      // MTLSamplerBorderColorOpaqueWhite
        }
    }

    /// Convert HgiComponentSwizzle to MTLTextureSwizzle.
    pub fn get_component_swizzle(swizzle: HgiComponentSwizzle) -> MtlTextureSwizzle {
        match swizzle {
            HgiComponentSwizzle::Zero => 0, // MTLTextureSwizzleZero
            HgiComponentSwizzle::One => 1,  // MTLTextureSwizzleOne
            HgiComponentSwizzle::R => 2,    // MTLTextureSwizzleRed
            HgiComponentSwizzle::G => 3,    // MTLTextureSwizzleGreen
            HgiComponentSwizzle::B => 4,    // MTLTextureSwizzleBlue
            HgiComponentSwizzle::A => 5,    // MTLTextureSwizzleAlpha
        }
    }

    /// Convert HgiPrimitiveType to MTLPrimitiveTopologyClass.
    pub fn get_primitive_class(prim_type: HgiPrimitiveType) -> MtlPrimitiveTopologyClass {
        match prim_type {
            HgiPrimitiveType::PointList => 1, // MTLPrimitiveTopologyClassPoint
            HgiPrimitiveType::LineList => 2,  // MTLPrimitiveTopologyClassLine
            HgiPrimitiveType::LineStrip => 2, // MTLPrimitiveTopologyClassLine
            HgiPrimitiveType::TriangleList => 3, // MTLPrimitiveTopologyClassTriangle
            HgiPrimitiveType::PatchList => 3, // MTLPrimitiveTopologyClassTriangle
            HgiPrimitiveType::LineListWithAdjacency => 0, // MTLPrimitiveTopologyClassUnspecified (matches C++)
        }
    }

    /// Convert HgiPrimitiveType to MTLPrimitiveType.
    pub fn get_primitive_type(prim_type: HgiPrimitiveType) -> MtlPrimitiveType {
        match prim_type {
            HgiPrimitiveType::PointList => 0,    // MTLPrimitiveTypePoint
            HgiPrimitiveType::LineList => 1,     // MTLPrimitiveTypeLine
            HgiPrimitiveType::LineStrip => 2,    // MTLPrimitiveTypeLineStrip
            HgiPrimitiveType::TriangleList => 3, // MTLPrimitiveTypeTriangle
            HgiPrimitiveType::PatchList => 3, // MTLPrimitiveTypeTriangle (invalid marker, matches C++)
            HgiPrimitiveType::LineListWithAdjacency => 3, // MTLPrimitiveTypeTriangle (invalid marker, matches C++)
        }
    }

    /// Convert HgiColorMask to MTLColorWriteMask.
    pub fn get_color_write_mask(mask: HgiColorMask) -> MtlColorWriteMask {
        let mut result: u32 = 0;
        if mask.contains(HgiColorMask::RED) {
            result |= 1 << 3; // MTLColorWriteMaskRed
        }
        if mask.contains(HgiColorMask::GREEN) {
            result |= 1 << 2; // MTLColorWriteMaskGreen
        }
        if mask.contains(HgiColorMask::BLUE) {
            result |= 1 << 1; // MTLColorWriteMaskBlue
        }
        if mask.contains(HgiColorMask::ALPHA) {
            result |= 1 << 0; // MTLColorWriteMaskAlpha
        }
        result
    }
}
