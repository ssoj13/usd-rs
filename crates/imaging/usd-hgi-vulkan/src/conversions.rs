//! HGI to Vulkan type conversion tables.
//!
//! Port of pxr/imaging/hgiVulkan/conversions.cpp/.h
//!
//! All functions are pure static lookups — no Vulkan runtime required.

use ash::vk;
use usd_hgi::{
    HgiAttachmentLoadOp, HgiAttachmentStoreOp, HgiBindResourceType, HgiBlendFactor, HgiBlendOp,
    HgiBorderColor, HgiBufferUsage, HgiCompareFunction, HgiComponentSwizzle, HgiCullMode,
    HgiFormat, HgiMipFilter, HgiPolygonMode, HgiPrimitiveType, HgiSampleCount,
    HgiSamplerAddressMode, HgiSamplerFilter, HgiShaderStage, HgiTextureType, HgiTextureUsage,
    HgiWinding,
};

/// Converts between HGI types and their Vulkan equivalents.
pub struct HgiVulkanConversions;

// ---------------------------------------------------------------------------
// Format table: HgiFormat (index) -> VkFormat
//
// The table is indexed by HgiFormat discriminant (0-based). Must stay in sync
// with the HgiFormat enum ordering. C++ validates with static_assert that
// HgiFormatCount==35; our enum has 36 entries (adds PackedD16Unorm at end),
// so we include it mapped to D16_UNORM.
// ---------------------------------------------------------------------------
const FORMAT_TABLE: &[(u32, vk::Format)] = &[
    // idx 0  UNorm8
    (0, vk::Format::R8_UNORM),
    // idx 1  UNorm8Vec2
    (1, vk::Format::R8G8_UNORM),
    // idx 2  UNorm8Vec4  (Vec3 unsupported in HgiFormat)
    (2, vk::Format::R8G8B8A8_UNORM),
    // idx 3  SNorm8
    (3, vk::Format::R8_SNORM),
    // idx 4  SNorm8Vec2
    (4, vk::Format::R8G8_SNORM),
    // idx 5  SNorm8Vec4
    (5, vk::Format::R8G8B8A8_SNORM),
    // idx 6  Float16
    (6, vk::Format::R16_SFLOAT),
    // idx 7  Float16Vec2
    (7, vk::Format::R16G16_SFLOAT),
    // idx 8  Float16Vec3
    (8, vk::Format::R16G16B16_SFLOAT),
    // idx 9  Float16Vec4
    (9, vk::Format::R16G16B16A16_SFLOAT),
    // idx 10 Float32
    (10, vk::Format::R32_SFLOAT),
    // idx 11 Float32Vec2
    (11, vk::Format::R32G32_SFLOAT),
    // idx 12 Float32Vec3
    (12, vk::Format::R32G32B32_SFLOAT),
    // idx 13 Float32Vec4
    (13, vk::Format::R32G32B32A32_SFLOAT),
    // idx 14 Int16
    (14, vk::Format::R16_SINT),
    // idx 15 Int16Vec2
    (15, vk::Format::R16G16_SINT),
    // idx 16 Int16Vec3
    (16, vk::Format::R16G16B16_SINT),
    // idx 17 Int16Vec4
    (17, vk::Format::R16G16B16A16_SINT),
    // idx 18 UInt16
    (18, vk::Format::R16_UINT),
    // idx 19 UInt16Vec2
    (19, vk::Format::R16G16_UINT),
    // idx 20 UInt16Vec3
    (20, vk::Format::R16G16B16_UINT),
    // idx 21 UInt16Vec4
    (21, vk::Format::R16G16B16A16_UINT),
    // idx 22 Int32
    (22, vk::Format::R32_SINT),
    // idx 23 Int32Vec2
    (23, vk::Format::R32G32_SINT),
    // idx 24 Int32Vec3
    (24, vk::Format::R32G32B32_SINT),
    // idx 25 Int32Vec4
    (25, vk::Format::R32G32B32A32_SINT),
    // idx 26 UNorm8Vec4srgb
    (26, vk::Format::R8G8B8A8_SRGB),
    // idx 27 BC6FloatVec3
    (27, vk::Format::BC6H_SFLOAT_BLOCK),
    // idx 28 BC6UFloatVec3
    (28, vk::Format::BC6H_UFLOAT_BLOCK),
    // idx 29 BC7UNorm8Vec4
    (29, vk::Format::BC7_UNORM_BLOCK),
    // idx 30 BC7UNorm8Vec4srgb
    (30, vk::Format::BC7_SRGB_BLOCK),
    // idx 31 BC1UNorm8Vec4
    (31, vk::Format::BC1_RGBA_UNORM_BLOCK),
    // idx 32 BC3UNorm8Vec4
    (32, vk::Format::BC3_UNORM_BLOCK),
    // idx 33 Float32UInt8 (depth-stencil)
    (33, vk::Format::D32_SFLOAT_S8_UINT),
    // idx 34 PackedInt1010102
    (34, vk::Format::A2B10G10R10_SNORM_PACK32),
    // idx 35 PackedD16Unorm (Rust-only addition)
    (35, vk::Format::D16_UNORM),
];

/// Image layout format qualifier strings for GLSL image unit format layout qualifiers.
///
/// Indexed by HgiFormat discriminant. Empty string means "no supported layout qualifier"
/// (C++ defaults to rgba16f in that case, index 9 = Float16Vec4).
const IMAGE_LAYOUT_QUALIFIERS: &[&str] = &[
    "r8",          // 0  UNorm8
    "rg8",         // 1  UNorm8Vec2
    "rgba8",       // 2  UNorm8Vec4
    "r8_snorm",    // 3  SNorm8
    "rg8_snorm",   // 4  SNorm8Vec2
    "rgba8_snorm", // 5  SNorm8Vec4
    "r16f",        // 6  Float16
    "rg16f",       // 7  Float16Vec2
    "",            // 8  Float16Vec3  (no layout qualifier)
    "rgba16f",     // 9  Float16Vec4
    "r32f",        // 10 Float32
    "rg32f",       // 11 Float32Vec2
    "",            // 12 Float32Vec3  (no layout qualifier)
    "rgba32f",     // 13 Float32Vec4
    "r16i",        // 14 Int16
    "rg16i",       // 15 Int16Vec2
    "",            // 16 Int16Vec3    (no layout qualifier)
    "rgba16i",     // 17 Int16Vec4
    "r16ui",       // 18 UInt16
    "rg16ui",      // 19 UInt16Vec2
    "",            // 20 UInt16Vec3   (no layout qualifier)
    "rgba16ui",    // 21 UInt16Vec4
    "r32i",        // 22 Int32
    "rg32i",       // 23 Int32Vec2
    "",            // 24 Int32Vec3    (no layout qualifier)
    "rgba32i",     // 25 Int32Vec4
    "",            // 26 UNorm8Vec4srgb
    "",            // 27 BC6FloatVec3
    "",            // 28 BC6UFloatVec3
    "",            // 29 BC7UNorm8Vec4
    "",            // 30 BC7UNorm8Vec4srgb
    "",            // 31 BC1UNorm8Vec4
    "",            // 32 BC3UNorm8Vec4
    "",            // 33 Float32UInt8
    "",            // 34 PackedInt1010102
    "",            // 35 PackedD16Unorm
];

impl HgiVulkanConversions {
    /// Converts an HgiFormat to the corresponding VkFormat.
    ///
    /// When `depth_format` is true, Float32 maps to D32_SFLOAT and
    /// Float32UInt8 maps to D32_SFLOAT_S8_UINT (matching C++ special-case).
    pub fn get_format(in_format: HgiFormat, depth_format: bool) -> vk::Format {
        if in_format == HgiFormat::Invalid {
            log::error!("HgiVulkanConversions::get_format called with HgiFormatInvalid");
            return vk::Format::UNDEFINED;
        }

        let idx = in_format as i32;
        if idx < 0 || idx as usize >= FORMAT_TABLE.len() {
            log::error!("HgiVulkanConversions::get_format: out-of-range HgiFormat {idx}");
            return vk::Format::UNDEFINED;
        }

        let mut vk_format = FORMAT_TABLE[idx as usize].1;

        // C++ special-case: plain float depth textures need their own VkFormat.
        if depth_format {
            if in_format == HgiFormat::Float32 {
                vk_format = vk::Format::D32_SFLOAT;
            } else if in_format == HgiFormat::Float32UInt8 {
                vk_format = vk::Format::D32_SFLOAT_S8_UINT;
            }
        }

        vk_format
    }

    /// Reverse-lookup: converts a VkFormat back to HgiFormat.
    ///
    /// BGRA8_UNORM is treated as RGBA8_UNORM because HgiFormat has no BGRA variant
    /// (matches C++ comment about native window swapchain).
    pub fn get_hgi_format(in_format: vk::Format) -> HgiFormat {
        if in_format == vk::Format::UNDEFINED {
            log::error!("HgiVulkanConversions::get_hgi_format called with VK_FORMAT_UNDEFINED");
            return HgiFormat::Invalid;
        }

        // BGRA swapchain surface -> treat as RGBA (no BGRA in HgiFormat)
        if in_format == vk::Format::B8G8R8A8_UNORM {
            return HgiFormat::UNorm8Vec4;
        }

        for (hgi_idx, vk_fmt) in FORMAT_TABLE {
            if *vk_fmt == in_format {
                return Self::idx_to_hgi_format(*hgi_idx);
            }
        }

        log::error!(
            "HgiVulkanConversions::get_hgi_format: missing format table entry for {in_format:?}"
        );
        HgiFormat::Invalid
    }

    /// Converts HgiTextureUsage flags to VkImageAspectFlags.
    pub fn get_image_aspect_flag(usage: HgiTextureUsage) -> vk::ImageAspectFlags {
        let has_depth = usage.contains(HgiTextureUsage::DEPTH_TARGET);
        let has_stencil = usage.contains(HgiTextureUsage::STENCIL_TARGET);

        if has_depth && has_stencil {
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        } else if has_depth {
            vk::ImageAspectFlags::DEPTH
        } else if has_stencil {
            vk::ImageAspectFlags::STENCIL
        } else {
            vk::ImageAspectFlags::COLOR
        }
    }

    /// Converts HgiTextureUsage flags to VkImageUsageFlags.
    ///
    /// Always adds TRANSFER_SRC and TRANSFER_DST bits (matching C++).
    pub fn get_texture_usage(tu: HgiTextureUsage) -> vk::ImageUsageFlags {
        let mut flags = vk::ImageUsageFlags::empty();

        if tu.contains(HgiTextureUsage::COLOR_TARGET) {
            flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
        }
        if tu.contains(HgiTextureUsage::DEPTH_TARGET) {
            flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
        }
        if tu.contains(HgiTextureUsage::STENCIL_TARGET) {
            flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
        }
        if tu.contains(HgiTextureUsage::SHADER_READ) {
            flags |= vk::ImageUsageFlags::SAMPLED;
        }
        if tu.contains(HgiTextureUsage::SHADER_WRITE) {
            flags |= vk::ImageUsageFlags::STORAGE;
        }

        if flags.is_empty() {
            log::error!(
                "HgiVulkanConversions::get_texture_usage: missing texture usage entry for {tu:?}"
            );
            flags = vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::STORAGE;
        }

        // C++ always adds transfer bits for blitting / mip generation.
        flags | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST
    }

    /// Converts HgiTextureUsage flags to VkFormatFeatureFlags2.
    pub fn get_format_feature2(tu: HgiTextureUsage) -> vk::FormatFeatureFlags2 {
        let mut flags = vk::FormatFeatureFlags2::empty();

        if tu.contains(HgiTextureUsage::COLOR_TARGET) {
            flags |= vk::FormatFeatureFlags2::COLOR_ATTACHMENT;
        }
        if tu.contains(HgiTextureUsage::DEPTH_TARGET) {
            flags |= vk::FormatFeatureFlags2::DEPTH_STENCIL_ATTACHMENT;
        }
        if tu.contains(HgiTextureUsage::STENCIL_TARGET) {
            flags |= vk::FormatFeatureFlags2::DEPTH_STENCIL_ATTACHMENT;
        }
        if tu.contains(HgiTextureUsage::SHADER_READ) {
            flags |= vk::FormatFeatureFlags2::SAMPLED_IMAGE;
        }
        if tu.contains(HgiTextureUsage::SHADER_WRITE) {
            flags |= vk::FormatFeatureFlags2::STORAGE_IMAGE;
        }

        if flags.is_empty() {
            log::error!(
                "HgiVulkanConversions::get_format_feature2: missing texture usage entry for {tu:?}"
            );
        }

        flags
    }

    /// Converts HgiAttachmentLoadOp to VkAttachmentLoadOp.
    pub fn get_load_op(op: HgiAttachmentLoadOp) -> vk::AttachmentLoadOp {
        match op {
            HgiAttachmentLoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
            HgiAttachmentLoadOp::Clear => vk::AttachmentLoadOp::CLEAR,
            HgiAttachmentLoadOp::Load => vk::AttachmentLoadOp::LOAD,
        }
    }

    /// Converts HgiAttachmentStoreOp to VkAttachmentStoreOp.
    pub fn get_store_op(op: HgiAttachmentStoreOp) -> vk::AttachmentStoreOp {
        match op {
            HgiAttachmentStoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
            HgiAttachmentStoreOp::Store => vk::AttachmentStoreOp::STORE,
        }
    }

    /// Converts HgiSampleCount to VkSampleCountFlagBits.
    pub fn get_sample_count(sc: HgiSampleCount) -> vk::SampleCountFlags {
        match sc {
            HgiSampleCount::Count1 => vk::SampleCountFlags::TYPE_1,
            HgiSampleCount::Count2 => vk::SampleCountFlags::TYPE_2,
            HgiSampleCount::Count4 => vk::SampleCountFlags::TYPE_4,
            HgiSampleCount::Count8 => vk::SampleCountFlags::TYPE_8,
            HgiSampleCount::Count16 => vk::SampleCountFlags::TYPE_16,
        }
    }

    /// Converts HgiShaderStage bitflags to VkShaderStageFlags.
    pub fn get_shader_stages(ss: HgiShaderStage) -> vk::ShaderStageFlags {
        let mut flags = vk::ShaderStageFlags::empty();

        if ss.contains(HgiShaderStage::VERTEX) {
            flags |= vk::ShaderStageFlags::VERTEX;
        }
        if ss.contains(HgiShaderStage::FRAGMENT) {
            flags |= vk::ShaderStageFlags::FRAGMENT;
        }
        if ss.contains(HgiShaderStage::COMPUTE) {
            flags |= vk::ShaderStageFlags::COMPUTE;
        }
        if ss.contains(HgiShaderStage::TESSELLATION_CONTROL) {
            flags |= vk::ShaderStageFlags::TESSELLATION_CONTROL;
        }
        if ss.contains(HgiShaderStage::TESSELLATION_EVAL) {
            flags |= vk::ShaderStageFlags::TESSELLATION_EVALUATION;
        }
        if ss.contains(HgiShaderStage::GEOMETRY) {
            flags |= vk::ShaderStageFlags::GEOMETRY;
        }

        if flags.is_empty() {
            log::error!(
                "HgiVulkanConversions::get_shader_stages: missing shader stage table entry"
            );
        }

        flags
    }

    /// Converts HgiBufferUsage bitflags to VkBufferUsageFlags.
    pub fn get_buffer_usage(bu: HgiBufferUsage) -> vk::BufferUsageFlags {
        let mut flags = vk::BufferUsageFlags::empty();

        if bu.contains(HgiBufferUsage::UNIFORM) {
            flags |= vk::BufferUsageFlags::UNIFORM_BUFFER;
        }
        if bu.contains(HgiBufferUsage::INDEX32) {
            flags |= vk::BufferUsageFlags::INDEX_BUFFER;
        }
        if bu.contains(HgiBufferUsage::VERTEX) {
            flags |= vk::BufferUsageFlags::VERTEX_BUFFER;
        }
        if bu.contains(HgiBufferUsage::STORAGE) {
            flags |= vk::BufferUsageFlags::STORAGE_BUFFER;
        }
        if bu.contains(HgiBufferUsage::INDIRECT) {
            flags |= vk::BufferUsageFlags::INDIRECT_BUFFER;
        }
        if bu.contains(HgiBufferUsage::UPLOAD) {
            flags |= vk::BufferUsageFlags::TRANSFER_SRC;
        }

        if flags.is_empty() {
            log::error!("HgiVulkanConversions::get_buffer_usage: missing buffer usage table entry");
        }

        flags
    }

    /// Converts HgiCullMode to VkCullModeFlags.
    pub fn get_cull_mode(cm: HgiCullMode) -> vk::CullModeFlags {
        match cm {
            HgiCullMode::None => vk::CullModeFlags::NONE,
            HgiCullMode::Front => vk::CullModeFlags::FRONT,
            HgiCullMode::Back => vk::CullModeFlags::BACK,
            HgiCullMode::FrontAndBack => vk::CullModeFlags::FRONT_AND_BACK,
        }
    }

    /// Converts HgiPolygonMode to VkPolygonMode.
    pub fn get_polygon_mode(pm: HgiPolygonMode) -> vk::PolygonMode {
        match pm {
            HgiPolygonMode::Fill => vk::PolygonMode::FILL,
            HgiPolygonMode::Line => vk::PolygonMode::LINE,
            HgiPolygonMode::Point => vk::PolygonMode::POINT,
        }
    }

    /// Converts HgiWinding to VkFrontFace.
    ///
    /// Winding is intentionally flipped in HgiVulkan: see HgiVulkanGraphicsCmds::SetViewport.
    pub fn get_winding(wd: HgiWinding) -> vk::FrontFace {
        match wd {
            // Flipped: HGI Clockwise -> Vulkan CounterClockwise
            HgiWinding::Clockwise => vk::FrontFace::COUNTER_CLOCKWISE,
            HgiWinding::CounterClockwise => vk::FrontFace::CLOCKWISE,
        }
    }

    /// Converts HgiBindResourceType to VkDescriptorType.
    pub fn get_descriptor_type(rt: HgiBindResourceType) -> vk::DescriptorType {
        match rt {
            HgiBindResourceType::Sampler => vk::DescriptorType::SAMPLER,
            HgiBindResourceType::SampledImage => vk::DescriptorType::SAMPLED_IMAGE,
            HgiBindResourceType::CombinedSamplerImage => vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            HgiBindResourceType::StorageImage => vk::DescriptorType::STORAGE_IMAGE,
            HgiBindResourceType::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
            HgiBindResourceType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
            // TessFactors uses a storage buffer to carry tessellation factor data.
            HgiBindResourceType::TessFactors => vk::DescriptorType::STORAGE_BUFFER,
        }
    }

    /// Converts HgiBlendFactor to VkBlendFactor.
    pub fn get_blend_factor(bf: HgiBlendFactor) -> vk::BlendFactor {
        match bf {
            HgiBlendFactor::Zero => vk::BlendFactor::ZERO,
            HgiBlendFactor::One => vk::BlendFactor::ONE,
            HgiBlendFactor::SrcColor => vk::BlendFactor::SRC_COLOR,
            HgiBlendFactor::OneMinusSrcColor => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
            HgiBlendFactor::DstColor => vk::BlendFactor::DST_COLOR,
            HgiBlendFactor::OneMinusDstColor => vk::BlendFactor::ONE_MINUS_DST_COLOR,
            HgiBlendFactor::SrcAlpha => vk::BlendFactor::SRC_ALPHA,
            HgiBlendFactor::OneMinusSrcAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            HgiBlendFactor::DstAlpha => vk::BlendFactor::DST_ALPHA,
            HgiBlendFactor::OneMinusDstAlpha => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
            HgiBlendFactor::ConstantColor => vk::BlendFactor::CONSTANT_COLOR,
            HgiBlendFactor::OneMinusConstantColor => vk::BlendFactor::ONE_MINUS_CONSTANT_COLOR,
            HgiBlendFactor::ConstantAlpha => vk::BlendFactor::CONSTANT_ALPHA,
            HgiBlendFactor::OneMinusConstantAlpha => vk::BlendFactor::ONE_MINUS_CONSTANT_ALPHA,
            HgiBlendFactor::SrcAlphaSaturate => vk::BlendFactor::SRC_ALPHA_SATURATE,
            HgiBlendFactor::Src1Color => vk::BlendFactor::SRC1_COLOR,
            HgiBlendFactor::OneMinusSrc1Color => vk::BlendFactor::ONE_MINUS_SRC1_COLOR,
            HgiBlendFactor::Src1Alpha => vk::BlendFactor::SRC1_ALPHA,
            HgiBlendFactor::OneMinusSrc1Alpha => vk::BlendFactor::ONE_MINUS_SRC1_ALPHA,
        }
    }

    /// Converts HgiBlendOp to VkBlendOp.
    pub fn get_blend_equation(bo: HgiBlendOp) -> vk::BlendOp {
        match bo {
            HgiBlendOp::Add => vk::BlendOp::ADD,
            HgiBlendOp::Subtract => vk::BlendOp::SUBTRACT,
            HgiBlendOp::ReverseSubtract => vk::BlendOp::REVERSE_SUBTRACT,
            HgiBlendOp::Min => vk::BlendOp::MIN,
            HgiBlendOp::Max => vk::BlendOp::MAX,
        }
    }

    /// Converts HgiCompareFunction to VkCompareOp.
    pub fn get_depth_compare_function(cf: HgiCompareFunction) -> vk::CompareOp {
        match cf {
            HgiCompareFunction::Never => vk::CompareOp::NEVER,
            HgiCompareFunction::Less => vk::CompareOp::LESS,
            HgiCompareFunction::Equal => vk::CompareOp::EQUAL,
            HgiCompareFunction::LEqual => vk::CompareOp::LESS_OR_EQUAL,
            HgiCompareFunction::Greater => vk::CompareOp::GREATER,
            HgiCompareFunction::NotEqual => vk::CompareOp::NOT_EQUAL,
            HgiCompareFunction::GEqual => vk::CompareOp::GREATER_OR_EQUAL,
            HgiCompareFunction::Always => vk::CompareOp::ALWAYS,
        }
    }

    /// Converts HgiTextureType to VkImageType.
    ///
    /// Both Cubemap and 2DArray use VK_IMAGE_TYPE_2D (matching C++).
    pub fn get_texture_type(tt: HgiTextureType) -> vk::ImageType {
        match tt {
            HgiTextureType::Texture1D => vk::ImageType::TYPE_1D,
            HgiTextureType::Texture2D => vk::ImageType::TYPE_2D,
            HgiTextureType::Texture3D => vk::ImageType::TYPE_3D,
            HgiTextureType::Cubemap => vk::ImageType::TYPE_2D,
            HgiTextureType::Texture1DArray => vk::ImageType::TYPE_1D,
            HgiTextureType::Texture2DArray => vk::ImageType::TYPE_2D,
        }
    }

    /// Converts HgiTextureType to VkImageViewType.
    pub fn get_texture_view_type(tt: HgiTextureType) -> vk::ImageViewType {
        match tt {
            HgiTextureType::Texture1D => vk::ImageViewType::TYPE_1D,
            HgiTextureType::Texture2D => vk::ImageViewType::TYPE_2D,
            HgiTextureType::Texture3D => vk::ImageViewType::TYPE_3D,
            HgiTextureType::Cubemap => vk::ImageViewType::CUBE,
            HgiTextureType::Texture1DArray => vk::ImageViewType::TYPE_1D_ARRAY,
            HgiTextureType::Texture2DArray => vk::ImageViewType::TYPE_2D_ARRAY,
        }
    }

    /// Converts HgiSamplerAddressMode to VkSamplerAddressMode.
    pub fn get_sampler_address_mode(a: HgiSamplerAddressMode) -> vk::SamplerAddressMode {
        match a {
            HgiSamplerAddressMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
            HgiSamplerAddressMode::MirrorClampToEdge => {
                vk::SamplerAddressMode::MIRROR_CLAMP_TO_EDGE
            }
            HgiSamplerAddressMode::Repeat => vk::SamplerAddressMode::REPEAT,
            HgiSamplerAddressMode::MirrorRepeat => vk::SamplerAddressMode::MIRRORED_REPEAT,
            HgiSamplerAddressMode::ClampToBorderColor => vk::SamplerAddressMode::CLAMP_TO_BORDER,
        }
    }

    /// Converts HgiSamplerFilter to VkFilter (min/mag filter).
    pub fn get_min_mag_filter(mf: HgiSamplerFilter) -> vk::Filter {
        match mf {
            HgiSamplerFilter::Nearest => vk::Filter::NEAREST,
            HgiSamplerFilter::Linear => vk::Filter::LINEAR,
        }
    }

    /// Converts HgiMipFilter to VkSamplerMipmapMode.
    ///
    /// NotMipmapped maps to NEAREST (unused in practice — callers set maxLod=0).
    pub fn get_mip_filter(mf: HgiMipFilter) -> vk::SamplerMipmapMode {
        match mf {
            HgiMipFilter::NotMipmapped => vk::SamplerMipmapMode::NEAREST,
            HgiMipFilter::Nearest => vk::SamplerMipmapMode::NEAREST,
            HgiMipFilter::Linear => vk::SamplerMipmapMode::LINEAR,
        }
    }

    /// Converts HgiBorderColor to VkBorderColor.
    pub fn get_border_color(bc: HgiBorderColor) -> vk::BorderColor {
        match bc {
            HgiBorderColor::TransparentBlack => vk::BorderColor::FLOAT_TRANSPARENT_BLACK,
            HgiBorderColor::OpaqueBlack => vk::BorderColor::FLOAT_OPAQUE_BLACK,
            HgiBorderColor::OpaqueWhite => vk::BorderColor::FLOAT_OPAQUE_WHITE,
        }
    }

    /// Converts HgiComponentSwizzle to VkComponentSwizzle.
    pub fn get_component_swizzle(cs: HgiComponentSwizzle) -> vk::ComponentSwizzle {
        match cs {
            HgiComponentSwizzle::Zero => vk::ComponentSwizzle::ZERO,
            HgiComponentSwizzle::One => vk::ComponentSwizzle::ONE,
            HgiComponentSwizzle::R => vk::ComponentSwizzle::R,
            HgiComponentSwizzle::G => vk::ComponentSwizzle::G,
            HgiComponentSwizzle::B => vk::ComponentSwizzle::B,
            HgiComponentSwizzle::A => vk::ComponentSwizzle::A,
        }
    }

    /// Converts HgiPrimitiveType to VkPrimitiveTopology.
    pub fn get_primitive_type(pt: HgiPrimitiveType) -> vk::PrimitiveTopology {
        match pt {
            HgiPrimitiveType::PointList => vk::PrimitiveTopology::POINT_LIST,
            HgiPrimitiveType::LineList => vk::PrimitiveTopology::LINE_LIST,
            HgiPrimitiveType::LineStrip => vk::PrimitiveTopology::LINE_STRIP,
            HgiPrimitiveType::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
            HgiPrimitiveType::PatchList => vk::PrimitiveTopology::PATCH_LIST,
            HgiPrimitiveType::LineListWithAdjacency => {
                vk::PrimitiveTopology::LINE_LIST_WITH_ADJACENCY
            }
        }
    }

    /// Returns the GLSL image unit format layout qualifier string for the given HgiFormat.
    ///
    /// If the format has no supported layout qualifier, logs a warning and
    /// returns "rgba16f" (Float16Vec4, index 9) as the C++ fallback.
    pub fn get_image_layout_format_qualifier(in_format: HgiFormat) -> String {
        let idx = in_format as i32;
        if idx >= 0 && (idx as usize) < IMAGE_LAYOUT_QUALIFIERS.len() {
            let qualifier = IMAGE_LAYOUT_QUALIFIERS[idx as usize];
            if qualifier.is_empty() {
                log::warn!(
                    "Given HgiFormat is not a supported image unit format, defaulting to rgba16f"
                );
                // C++ fallback: index 9 = Float16Vec4 = "rgba16f"
                return IMAGE_LAYOUT_QUALIFIERS[9].to_string();
            }
            return qualifier.to_string();
        }
        log::warn!("HgiVulkanConversions::get_image_layout_format_qualifier: invalid format {idx}");
        IMAGE_LAYOUT_QUALIFIERS[9].to_string()
    }

    // -----------------------------------------------------------------------
    // Private helper
    // -----------------------------------------------------------------------

    /// Converts a raw FORMAT_TABLE index back to HgiFormat.
    ///
    /// Must stay in sync with the HgiFormat enum discriminant values.
    fn idx_to_hgi_format(idx: u32) -> HgiFormat {
        match idx {
            0 => HgiFormat::UNorm8,
            1 => HgiFormat::UNorm8Vec2,
            2 => HgiFormat::UNorm8Vec4,
            3 => HgiFormat::SNorm8,
            4 => HgiFormat::SNorm8Vec2,
            5 => HgiFormat::SNorm8Vec4,
            6 => HgiFormat::Float16,
            7 => HgiFormat::Float16Vec2,
            8 => HgiFormat::Float16Vec3,
            9 => HgiFormat::Float16Vec4,
            10 => HgiFormat::Float32,
            11 => HgiFormat::Float32Vec2,
            12 => HgiFormat::Float32Vec3,
            13 => HgiFormat::Float32Vec4,
            14 => HgiFormat::Int16,
            15 => HgiFormat::Int16Vec2,
            16 => HgiFormat::Int16Vec3,
            17 => HgiFormat::Int16Vec4,
            18 => HgiFormat::UInt16,
            19 => HgiFormat::UInt16Vec2,
            20 => HgiFormat::UInt16Vec3,
            21 => HgiFormat::UInt16Vec4,
            22 => HgiFormat::Int32,
            23 => HgiFormat::Int32Vec2,
            24 => HgiFormat::Int32Vec3,
            25 => HgiFormat::Int32Vec4,
            26 => HgiFormat::UNorm8Vec4srgb,
            27 => HgiFormat::BC6FloatVec3,
            28 => HgiFormat::BC6UFloatVec3,
            29 => HgiFormat::BC7UNorm8Vec4,
            30 => HgiFormat::BC7UNorm8Vec4srgb,
            31 => HgiFormat::BC1UNorm8Vec4,
            32 => HgiFormat::BC3UNorm8Vec4,
            33 => HgiFormat::Float32UInt8,
            34 => HgiFormat::PackedInt1010102,
            35 => HgiFormat::PackedD16Unorm,
            _ => {
                log::error!("HgiVulkanConversions::idx_to_hgi_format: unknown index {idx}");
                HgiFormat::Invalid
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_roundtrip() {
        // Every format in the table should round-trip (excluding depth special-cases)
        let cases = [
            HgiFormat::UNorm8,
            HgiFormat::UNorm8Vec2,
            HgiFormat::UNorm8Vec4,
            HgiFormat::Float32Vec4,
            HgiFormat::BC7UNorm8Vec4,
            HgiFormat::PackedInt1010102,
        ];
        for fmt in cases {
            let vk = HgiVulkanConversions::get_format(fmt, false);
            let back = HgiVulkanConversions::get_hgi_format(vk);
            assert_eq!(back, fmt, "roundtrip failed for {fmt:?}");
        }
    }

    #[test]
    fn test_depth_format_special_case() {
        assert_eq!(
            HgiVulkanConversions::get_format(HgiFormat::Float32, true),
            vk::Format::D32_SFLOAT
        );
        assert_eq!(
            HgiVulkanConversions::get_format(HgiFormat::Float32, false),
            vk::Format::R32_SFLOAT
        );
        assert_eq!(
            HgiVulkanConversions::get_format(HgiFormat::Float32UInt8, true),
            vk::Format::D32_SFLOAT_S8_UINT
        );
    }

    #[test]
    fn test_bgra_swapchain_mapping() {
        assert_eq!(
            HgiVulkanConversions::get_hgi_format(vk::Format::B8G8R8A8_UNORM),
            HgiFormat::UNorm8Vec4
        );
    }

    #[test]
    fn test_invalid_format() {
        assert_eq!(
            HgiVulkanConversions::get_format(HgiFormat::Invalid, false),
            vk::Format::UNDEFINED
        );
        assert_eq!(
            HgiVulkanConversions::get_hgi_format(vk::Format::UNDEFINED),
            HgiFormat::Invalid
        );
    }

    #[test]
    fn test_image_aspect_flags() {
        assert_eq!(
            HgiVulkanConversions::get_image_aspect_flag(HgiTextureUsage::COLOR_TARGET),
            vk::ImageAspectFlags::COLOR
        );
        assert_eq!(
            HgiVulkanConversions::get_image_aspect_flag(HgiTextureUsage::DEPTH_TARGET),
            vk::ImageAspectFlags::DEPTH
        );
        assert_eq!(
            HgiVulkanConversions::get_image_aspect_flag(
                HgiTextureUsage::DEPTH_TARGET | HgiTextureUsage::STENCIL_TARGET
            ),
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        );
    }

    #[test]
    fn test_texture_usage_always_has_transfer_bits() {
        let flags = HgiVulkanConversions::get_texture_usage(HgiTextureUsage::COLOR_TARGET);
        assert!(flags.contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(flags.contains(vk::ImageUsageFlags::TRANSFER_DST));
        assert!(flags.contains(vk::ImageUsageFlags::COLOR_ATTACHMENT));
    }

    #[test]
    fn test_winding_is_flipped() {
        // C++ intentionally flips winding for Vulkan's coordinate system.
        assert_eq!(
            HgiVulkanConversions::get_winding(HgiWinding::Clockwise),
            vk::FrontFace::COUNTER_CLOCKWISE
        );
        assert_eq!(
            HgiVulkanConversions::get_winding(HgiWinding::CounterClockwise),
            vk::FrontFace::CLOCKWISE
        );
    }

    #[test]
    fn test_tess_factors_uses_storage_buffer() {
        assert_eq!(
            HgiVulkanConversions::get_descriptor_type(HgiBindResourceType::TessFactors),
            vk::DescriptorType::STORAGE_BUFFER
        );
    }

    #[test]
    fn test_image_layout_format_qualifier() {
        assert_eq!(
            HgiVulkanConversions::get_image_layout_format_qualifier(HgiFormat::Float32Vec4),
            "rgba32f"
        );
        assert_eq!(
            HgiVulkanConversions::get_image_layout_format_qualifier(HgiFormat::UNorm8),
            "r8"
        );
        // Float16Vec3 has no qualifier -> falls back to rgba16f
        assert_eq!(
            HgiVulkanConversions::get_image_layout_format_qualifier(HgiFormat::Float16Vec3),
            "rgba16f"
        );
    }

    #[test]
    fn test_sample_count() {
        assert_eq!(
            HgiVulkanConversions::get_sample_count(HgiSampleCount::Count1),
            vk::SampleCountFlags::TYPE_1
        );
        assert_eq!(
            HgiVulkanConversions::get_sample_count(HgiSampleCount::Count16),
            vk::SampleCountFlags::TYPE_16
        );
    }

    #[test]
    fn test_shader_stages_bitmask() {
        let stages = HgiShaderStage::VERTEX | HgiShaderStage::FRAGMENT;
        let vk = HgiVulkanConversions::get_shader_stages(stages);
        assert!(vk.contains(vk::ShaderStageFlags::VERTEX));
        assert!(vk.contains(vk::ShaderStageFlags::FRAGMENT));
        assert!(!vk.contains(vk::ShaderStageFlags::COMPUTE));
    }

    #[test]
    fn test_buffer_usage_bitmask() {
        let usage = HgiBufferUsage::VERTEX | HgiBufferUsage::UNIFORM;
        let vk = HgiVulkanConversions::get_buffer_usage(usage);
        assert!(vk.contains(vk::BufferUsageFlags::VERTEX_BUFFER));
        assert!(vk.contains(vk::BufferUsageFlags::UNIFORM_BUFFER));
        assert!(!vk.contains(vk::BufferUsageFlags::INDEX_BUFFER));
    }

    #[test]
    fn test_texture_types() {
        assert_eq!(
            HgiVulkanConversions::get_texture_type(HgiTextureType::Cubemap),
            vk::ImageType::TYPE_2D
        );
        assert_eq!(
            HgiVulkanConversions::get_texture_view_type(HgiTextureType::Cubemap),
            vk::ImageViewType::CUBE
        );
        assert_eq!(
            HgiVulkanConversions::get_texture_view_type(HgiTextureType::Texture2DArray),
            vk::ImageViewType::TYPE_2D_ARRAY
        );
    }
}
