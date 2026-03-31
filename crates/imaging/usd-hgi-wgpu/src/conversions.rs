//! HGI enum <-> wgpu type conversion utilities.
//!
//! Central mapping between usd-hgi enums and wgpu types. Each function is
//! a pure mapping with no side effects, making them safe to call from any
//! context.

use usd_hgi::{
    HgiBindResourceType, HgiBlendFactor, HgiBlendOp, HgiBorderColor, HgiBufferUsage, HgiColorMask,
    HgiCompareFunction, HgiCullMode, HgiFormat, HgiPolygonMode, HgiPrimitiveType, HgiSampleCount,
    HgiSamplerAddressMode, HgiSamplerFilter, HgiShaderStage, HgiStencilOp, HgiTextureType,
    HgiTextureUsage, HgiVertexBufferStepFunction, HgiWinding,
};

// -- Buffer usage --

/// Map HgiBufferUsage flags to wgpu::BufferUsages.
pub fn to_wgpu_buffer_usages(usage: HgiBufferUsage) -> wgpu::BufferUsages {
    let mut out = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;

    if usage.contains(HgiBufferUsage::UNIFORM) {
        out |= wgpu::BufferUsages::UNIFORM;
    }
    if usage.contains(HgiBufferUsage::STORAGE) {
        out |= wgpu::BufferUsages::STORAGE;
    }
    if usage.contains(HgiBufferUsage::VERTEX) {
        out |= wgpu::BufferUsages::VERTEX;
    }
    if usage.contains(HgiBufferUsage::INDEX32) || usage.contains(HgiBufferUsage::INDEX16) {
        out |= wgpu::BufferUsages::INDEX;
    }
    if usage.contains(HgiBufferUsage::INDIRECT) {
        out |= wgpu::BufferUsages::INDIRECT;
    }

    out
}

// -- Texture format --

/// Map HgiFormat to wgpu::TextureFormat.
pub fn to_wgpu_texture_format(format: HgiFormat) -> wgpu::TextureFormat {
    match format {
        HgiFormat::UNorm8 => wgpu::TextureFormat::R8Unorm,
        HgiFormat::UNorm8Vec2 => wgpu::TextureFormat::Rg8Unorm,
        HgiFormat::UNorm8Vec4 => wgpu::TextureFormat::Rgba8Unorm,
        HgiFormat::SNorm8 => wgpu::TextureFormat::R8Snorm,
        HgiFormat::SNorm8Vec2 => wgpu::TextureFormat::Rg8Snorm,
        HgiFormat::SNorm8Vec4 => wgpu::TextureFormat::Rgba8Snorm,
        HgiFormat::Float16 => wgpu::TextureFormat::R16Float,
        HgiFormat::Float16Vec2 => wgpu::TextureFormat::Rg16Float,
        HgiFormat::Float16Vec4 => wgpu::TextureFormat::Rgba16Float,
        HgiFormat::Float32 => wgpu::TextureFormat::R32Float,
        HgiFormat::Float32Vec2 => wgpu::TextureFormat::Rg32Float,
        HgiFormat::Float32Vec4 => wgpu::TextureFormat::Rgba32Float,
        HgiFormat::Int16 => wgpu::TextureFormat::R16Sint,
        HgiFormat::Int16Vec2 => wgpu::TextureFormat::Rg16Sint,
        HgiFormat::Int16Vec4 => wgpu::TextureFormat::Rgba16Sint,
        HgiFormat::UInt16 => wgpu::TextureFormat::R16Uint,
        HgiFormat::UInt16Vec2 => wgpu::TextureFormat::Rg16Uint,
        HgiFormat::UInt16Vec4 => wgpu::TextureFormat::Rgba16Uint,
        HgiFormat::Int32 => wgpu::TextureFormat::R32Sint,
        HgiFormat::Int32Vec2 => wgpu::TextureFormat::Rg32Sint,
        HgiFormat::Int32Vec4 => wgpu::TextureFormat::Rgba32Sint,
        HgiFormat::UNorm8Vec4srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        HgiFormat::BC6FloatVec3 => wgpu::TextureFormat::Bc6hRgbFloat,
        HgiFormat::BC6UFloatVec3 => wgpu::TextureFormat::Bc6hRgbUfloat,
        HgiFormat::BC7UNorm8Vec4 => wgpu::TextureFormat::Bc7RgbaUnorm,
        HgiFormat::BC7UNorm8Vec4srgb => wgpu::TextureFormat::Bc7RgbaUnormSrgb,
        HgiFormat::BC1UNorm8Vec4 => wgpu::TextureFormat::Bc1RgbaUnorm,
        HgiFormat::BC3UNorm8Vec4 => wgpu::TextureFormat::Bc3RgbaUnorm,
        HgiFormat::Float32UInt8 => wgpu::TextureFormat::Depth32FloatStencil8,
        HgiFormat::PackedInt1010102 => wgpu::TextureFormat::Rgb10a2Unorm,
        HgiFormat::PackedD16Unorm => wgpu::TextureFormat::Depth16Unorm,
        // 3-component formats not natively supported in wgpu, pad to 4-component.
        // Use debug! to avoid log spam in render loops (P2-3 fix).
        HgiFormat::Float16Vec3 => {
            log::debug!("Float16Vec3 promoted to Rgba16Float (3-comp not supported in wgpu)");
            wgpu::TextureFormat::Rgba16Float
        }
        HgiFormat::Int16Vec3 => {
            log::debug!("Int16Vec3 promoted to Rgba16Sint");
            wgpu::TextureFormat::Rgba16Sint
        }
        HgiFormat::UInt16Vec3 => {
            log::debug!("UInt16Vec3 promoted to Rgba16Uint");
            wgpu::TextureFormat::Rgba16Uint
        }
        HgiFormat::Float32Vec3 => {
            log::debug!("Float32Vec3 promoted to Rgba32Float");
            wgpu::TextureFormat::Rgba32Float
        }
        HgiFormat::Int32Vec3 => {
            log::debug!("Int32Vec3 promoted to Rgba32Sint");
            wgpu::TextureFormat::Rgba32Sint
        }
        HgiFormat::Invalid => wgpu::TextureFormat::Rgba8Unorm, // fallback
    }
}

// -- Vertex format --

/// Map HgiFormat to wgpu::VertexFormat for vertex attributes.
pub fn to_wgpu_vertex_format(format: HgiFormat) -> wgpu::VertexFormat {
    match format {
        HgiFormat::Float32 => wgpu::VertexFormat::Float32,
        HgiFormat::Float32Vec2 => wgpu::VertexFormat::Float32x2,
        HgiFormat::Float32Vec3 => wgpu::VertexFormat::Float32x3,
        HgiFormat::Float32Vec4 => wgpu::VertexFormat::Float32x4,
        HgiFormat::Int32 => wgpu::VertexFormat::Sint32,
        HgiFormat::Int32Vec2 => wgpu::VertexFormat::Sint32x2,
        HgiFormat::Int32Vec3 => wgpu::VertexFormat::Sint32x3,
        HgiFormat::Int32Vec4 => wgpu::VertexFormat::Sint32x4,
        HgiFormat::Float16Vec2 => wgpu::VertexFormat::Float16x2,
        HgiFormat::Float16Vec4 => wgpu::VertexFormat::Float16x4,
        HgiFormat::UNorm8Vec2 => wgpu::VertexFormat::Unorm8x2,
        HgiFormat::UNorm8Vec4 => wgpu::VertexFormat::Unorm8x4,
        HgiFormat::SNorm8Vec2 => wgpu::VertexFormat::Snorm8x2,
        HgiFormat::SNorm8Vec4 => wgpu::VertexFormat::Snorm8x4,
        HgiFormat::Int16Vec2 => wgpu::VertexFormat::Sint16x2,
        HgiFormat::Int16Vec4 => wgpu::VertexFormat::Sint16x4,
        HgiFormat::UInt16Vec2 => wgpu::VertexFormat::Uint16x2,
        HgiFormat::UInt16Vec4 => wgpu::VertexFormat::Uint16x4,
        _ => wgpu::VertexFormat::Float32x4, // fallback
    }
}

// -- Texture usage --

/// Map HgiTextureUsage flags to wgpu::TextureUsages.
pub fn to_wgpu_texture_usages(usage: HgiTextureUsage) -> wgpu::TextureUsages {
    let mut out = wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC;

    if usage.contains(HgiTextureUsage::SHADER_READ) {
        out |= wgpu::TextureUsages::TEXTURE_BINDING;
    }
    if usage.contains(HgiTextureUsage::SHADER_WRITE) {
        out |= wgpu::TextureUsages::STORAGE_BINDING;
    }
    if usage.contains(HgiTextureUsage::COLOR_TARGET) {
        out |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    if usage.contains(HgiTextureUsage::DEPTH_TARGET)
        || usage.contains(HgiTextureUsage::STENCIL_TARGET)
    {
        out |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }

    out
}

// -- Texture dimension --

/// Map HgiTextureType to wgpu::TextureDimension.
pub fn to_wgpu_texture_dimension(tex_type: HgiTextureType) -> wgpu::TextureDimension {
    match tex_type {
        HgiTextureType::Texture1D | HgiTextureType::Texture1DArray => wgpu::TextureDimension::D1,
        HgiTextureType::Texture3D => wgpu::TextureDimension::D3,
        _ => wgpu::TextureDimension::D2,
    }
}

// -- Primitive topology --

/// Map HgiPrimitiveType to wgpu::PrimitiveTopology.
pub fn to_wgpu_primitive_topology(prim: HgiPrimitiveType) -> wgpu::PrimitiveTopology {
    match prim {
        HgiPrimitiveType::PointList => wgpu::PrimitiveTopology::PointList,
        HgiPrimitiveType::LineList => wgpu::PrimitiveTopology::LineList,
        HgiPrimitiveType::LineStrip => wgpu::PrimitiveTopology::LineStrip,
        HgiPrimitiveType::TriangleList => wgpu::PrimitiveTopology::TriangleList,
        // PatchList and LineListWithAdjacency have no direct wgpu equivalent
        HgiPrimitiveType::PatchList => {
            log::warn!("PatchList topology not supported in wgpu, falling back to TriangleList");
            wgpu::PrimitiveTopology::TriangleList
        }
        HgiPrimitiveType::LineListWithAdjacency => wgpu::PrimitiveTopology::LineList,
    }
}

// -- Cull mode --

/// Map HgiCullMode to wgpu::Face (Some = cull that face, None = no culling).
pub fn to_wgpu_cull_mode(cull: HgiCullMode) -> Option<wgpu::Face> {
    match cull {
        HgiCullMode::None => None,
        HgiCullMode::Front => Some(wgpu::Face::Front),
        HgiCullMode::Back => Some(wgpu::Face::Back),
        // wgpu doesn't have FrontAndBack; use Back as fallback
        HgiCullMode::FrontAndBack => {
            log::warn!("CullMode::FrontAndBack not supported in wgpu, using Back as fallback");
            Some(wgpu::Face::Back)
        }
    }
}

// -- Front face winding --

/// Map HgiWinding to wgpu::FrontFace.
pub fn to_wgpu_front_face(winding: HgiWinding) -> wgpu::FrontFace {
    match winding {
        HgiWinding::Clockwise => wgpu::FrontFace::Cw,
        HgiWinding::CounterClockwise => wgpu::FrontFace::Ccw,
    }
}

// -- Polygon mode --

/// Map HgiPolygonMode to wgpu::PolygonMode.
pub fn to_wgpu_polygon_mode(mode: HgiPolygonMode) -> wgpu::PolygonMode {
    match mode {
        HgiPolygonMode::Fill => wgpu::PolygonMode::Fill,
        HgiPolygonMode::Line => wgpu::PolygonMode::Line,
        HgiPolygonMode::Point => wgpu::PolygonMode::Point,
    }
}

// -- Compare function --

/// Map HgiCompareFunction to wgpu::CompareFunction.
pub fn to_wgpu_compare_fn(cmp: HgiCompareFunction) -> wgpu::CompareFunction {
    match cmp {
        HgiCompareFunction::Never => wgpu::CompareFunction::Never,
        HgiCompareFunction::Less => wgpu::CompareFunction::Less,
        HgiCompareFunction::Equal => wgpu::CompareFunction::Equal,
        HgiCompareFunction::LEqual => wgpu::CompareFunction::LessEqual,
        HgiCompareFunction::Greater => wgpu::CompareFunction::Greater,
        HgiCompareFunction::NotEqual => wgpu::CompareFunction::NotEqual,
        HgiCompareFunction::GEqual => wgpu::CompareFunction::GreaterEqual,
        HgiCompareFunction::Always => wgpu::CompareFunction::Always,
    }
}

// -- Stencil operations --

/// Map HgiStencilOp to wgpu::StencilOperation.
pub fn to_wgpu_stencil_op(op: HgiStencilOp) -> wgpu::StencilOperation {
    match op {
        HgiStencilOp::Keep => wgpu::StencilOperation::Keep,
        HgiStencilOp::Zero => wgpu::StencilOperation::Zero,
        HgiStencilOp::Replace => wgpu::StencilOperation::Replace,
        HgiStencilOp::IncrementClamp => wgpu::StencilOperation::IncrementClamp,
        HgiStencilOp::DecrementClamp => wgpu::StencilOperation::DecrementClamp,
        HgiStencilOp::Invert => wgpu::StencilOperation::Invert,
        HgiStencilOp::IncrementWrap => wgpu::StencilOperation::IncrementWrap,
        HgiStencilOp::DecrementWrap => wgpu::StencilOperation::DecrementWrap,
    }
}

// -- Blend operations --

/// Map HgiBlendOp to wgpu::BlendOperation.
pub fn to_wgpu_blend_op(op: HgiBlendOp) -> wgpu::BlendOperation {
    match op {
        HgiBlendOp::Add => wgpu::BlendOperation::Add,
        HgiBlendOp::Subtract => wgpu::BlendOperation::Subtract,
        HgiBlendOp::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        HgiBlendOp::Min => wgpu::BlendOperation::Min,
        HgiBlendOp::Max => wgpu::BlendOperation::Max,
    }
}

// -- Blend factors --

/// Map HgiBlendFactor to wgpu::BlendFactor.
pub fn to_wgpu_blend_factor(factor: HgiBlendFactor) -> wgpu::BlendFactor {
    match factor {
        HgiBlendFactor::Zero => wgpu::BlendFactor::Zero,
        HgiBlendFactor::One => wgpu::BlendFactor::One,
        HgiBlendFactor::SrcColor => wgpu::BlendFactor::Src,
        HgiBlendFactor::OneMinusSrcColor => wgpu::BlendFactor::OneMinusSrc,
        HgiBlendFactor::DstColor => wgpu::BlendFactor::Dst,
        HgiBlendFactor::OneMinusDstColor => wgpu::BlendFactor::OneMinusDst,
        HgiBlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
        HgiBlendFactor::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        HgiBlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
        HgiBlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        HgiBlendFactor::ConstantColor => wgpu::BlendFactor::Constant,
        HgiBlendFactor::OneMinusConstantColor => wgpu::BlendFactor::OneMinusConstant,
        HgiBlendFactor::SrcAlphaSaturate => wgpu::BlendFactor::SrcAlphaSaturated,
        HgiBlendFactor::Src1Color => wgpu::BlendFactor::Src1,
        HgiBlendFactor::OneMinusSrc1Color => wgpu::BlendFactor::OneMinusSrc1,
        HgiBlendFactor::Src1Alpha => wgpu::BlendFactor::Src1Alpha,
        HgiBlendFactor::OneMinusSrc1Alpha => wgpu::BlendFactor::OneMinusSrc1Alpha,
        // Constant alpha mapped to Constant (wgpu merges color/alpha constants)
        HgiBlendFactor::ConstantAlpha => wgpu::BlendFactor::Constant,
        HgiBlendFactor::OneMinusConstantAlpha => wgpu::BlendFactor::OneMinusConstant,
    }
}

// -- Color write mask --

/// Map HgiColorMask to wgpu::ColorWrites.
pub fn to_wgpu_color_writes(mask: HgiColorMask) -> wgpu::ColorWrites {
    let mut out = wgpu::ColorWrites::empty();
    if mask.contains(HgiColorMask::RED) {
        out |= wgpu::ColorWrites::RED;
    }
    if mask.contains(HgiColorMask::GREEN) {
        out |= wgpu::ColorWrites::GREEN;
    }
    if mask.contains(HgiColorMask::BLUE) {
        out |= wgpu::ColorWrites::BLUE;
    }
    if mask.contains(HgiColorMask::ALPHA) {
        out |= wgpu::ColorWrites::ALPHA;
    }
    out
}

// -- Sample count --

/// Map HgiSampleCount to u32 for wgpu multisample state.
pub fn to_wgpu_sample_count(count: HgiSampleCount) -> u32 {
    match count {
        HgiSampleCount::Count1 => 1,
        HgiSampleCount::Count2 => 2,
        HgiSampleCount::Count4 => 4,
        HgiSampleCount::Count8 => 8,
        HgiSampleCount::Count16 => 16,
    }
}

// -- Vertex step mode --

/// Map HgiVertexBufferStepFunction to wgpu::VertexStepMode.
pub fn to_wgpu_step_mode(step: HgiVertexBufferStepFunction) -> wgpu::VertexStepMode {
    match step {
        HgiVertexBufferStepFunction::PerVertex => wgpu::VertexStepMode::Vertex,
        HgiVertexBufferStepFunction::PerInstance => wgpu::VertexStepMode::Instance,
        // wgpu only supports Vertex and Instance; other modes use Vertex
        HgiVertexBufferStepFunction::Constant
        | HgiVertexBufferStepFunction::PerPatch
        | HgiVertexBufferStepFunction::PerPatchControlPoint
        | HgiVertexBufferStepFunction::PerDrawCommand => {
            log::warn!(
                "StepMode {:?} not supported in wgpu, falling back to Vertex",
                step
            );
            wgpu::VertexStepMode::Vertex
        }
    }
}

// -- Sampler --

/// Map HgiSamplerAddressMode to wgpu::AddressMode.
pub fn to_wgpu_address_mode(mode: HgiSamplerAddressMode) -> wgpu::AddressMode {
    match mode {
        HgiSamplerAddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        // wgpu doesn't have MirrorClampToEdge, fallback to MirrorRepeat
        HgiSamplerAddressMode::MirrorClampToEdge => {
            log::warn!("MirrorClampToEdge address mode not supported in wgpu, using MirrorRepeat");
            wgpu::AddressMode::MirrorRepeat
        }
        HgiSamplerAddressMode::Repeat => wgpu::AddressMode::Repeat,
        HgiSamplerAddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        HgiSamplerAddressMode::ClampToBorderColor => wgpu::AddressMode::ClampToBorder,
    }
}

/// Map HgiSamplerFilter to wgpu::FilterMode.
pub fn to_wgpu_filter_mode(filter: HgiSamplerFilter) -> wgpu::FilterMode {
    match filter {
        HgiSamplerFilter::Nearest => wgpu::FilterMode::Nearest,
        HgiSamplerFilter::Linear => wgpu::FilterMode::Linear,
    }
}

/// Map HgiBorderColor to wgpu::SamplerBorderColor.
pub fn to_wgpu_border_color(color: HgiBorderColor) -> wgpu::SamplerBorderColor {
    match color {
        HgiBorderColor::TransparentBlack => wgpu::SamplerBorderColor::TransparentBlack,
        HgiBorderColor::OpaqueBlack => wgpu::SamplerBorderColor::OpaqueBlack,
        HgiBorderColor::OpaqueWhite => wgpu::SamplerBorderColor::OpaqueWhite,
    }
}

// -- Shader stage visibility --

/// Map HgiShaderStage flags to wgpu::ShaderStages.
pub fn to_wgpu_shader_stages(stage: HgiShaderStage) -> wgpu::ShaderStages {
    let mut out = wgpu::ShaderStages::empty();
    if stage.contains(HgiShaderStage::VERTEX) {
        out |= wgpu::ShaderStages::VERTEX;
    }
    if stage.contains(HgiShaderStage::FRAGMENT) {
        out |= wgpu::ShaderStages::FRAGMENT;
    }
    if stage.contains(HgiShaderStage::COMPUTE) {
        out |= wgpu::ShaderStages::COMPUTE;
    }
    out
}

// -- Bind resource type --

/// Map HgiBindResourceType to wgpu::BindingType for buffers.
pub fn to_wgpu_buffer_binding_type(
    res_type: HgiBindResourceType,
    writable: bool,
) -> wgpu::BindingType {
    match res_type {
        HgiBindResourceType::UniformBuffer => wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        HgiBindResourceType::StorageBuffer => wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage {
                read_only: !writable,
            },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        _ => wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    }
}

/// Map HgiBindResourceType to wgpu::BindingType for textures.
///
/// Uses Rgba8Unorm as default format for StorageImage.
pub fn to_wgpu_texture_binding_type(
    res_type: HgiBindResourceType,
    writable: bool,
) -> wgpu::BindingType {
    to_wgpu_texture_binding_type_with_format(res_type, writable, HgiFormat::Invalid)
}

/// Map HgiBindResourceType to wgpu::BindingType for textures with explicit format.
///
/// For StorageImage, the format parameter is used to determine the actual storage format.
/// If format is Invalid, falls back to Rgba8Unorm.
pub fn to_wgpu_texture_binding_type_with_format(
    res_type: HgiBindResourceType,
    writable: bool,
    format: HgiFormat,
) -> wgpu::BindingType {
    match res_type {
        HgiBindResourceType::StorageImage => {
            let storage_format = if format == HgiFormat::Invalid {
                wgpu::TextureFormat::Rgba8Unorm
            } else {
                to_wgpu_texture_format(format)
            };
            wgpu::BindingType::StorageTexture {
                access: if writable {
                    wgpu::StorageTextureAccess::WriteOnly
                } else {
                    wgpu::StorageTextureAccess::ReadOnly
                },
                format: storage_format,
                view_dimension: wgpu::TextureViewDimension::D2,
            }
        }
        _ => wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
    }
}

// -- Depth format helpers --

/// Check if a given HgiFormat is a depth-only or depth-stencil format.
pub fn is_depth_format(format: HgiFormat) -> bool {
    matches!(
        format,
        HgiFormat::Float32 | HgiFormat::Float32UInt8 | HgiFormat::PackedD16Unorm
    )
}

/// Get the wgpu depth format for pipeline creation.
/// Returns None if format is not a depth format.
pub fn to_wgpu_depth_format(format: HgiFormat) -> Option<wgpu::TextureFormat> {
    match format {
        HgiFormat::Float32 => Some(wgpu::TextureFormat::Depth32Float),
        HgiFormat::Float32UInt8 => Some(wgpu::TextureFormat::Depth32FloatStencil8),
        // 16-bit normalized depth-only format
        HgiFormat::PackedD16Unorm => Some(wgpu::TextureFormat::Depth16Unorm),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_usages() {
        let usage = HgiBufferUsage::VERTEX | HgiBufferUsage::INDEX32;
        let wgpu_usage = to_wgpu_buffer_usages(usage);
        assert!(wgpu_usage.contains(wgpu::BufferUsages::VERTEX));
        assert!(wgpu_usage.contains(wgpu::BufferUsages::INDEX));
    }

    #[test]
    fn test_texture_format() {
        assert_eq!(
            to_wgpu_texture_format(HgiFormat::UNorm8Vec4),
            wgpu::TextureFormat::Rgba8Unorm
        );
        assert_eq!(
            to_wgpu_texture_format(HgiFormat::Float32UInt8),
            wgpu::TextureFormat::Depth32FloatStencil8
        );
    }

    #[test]
    fn test_blend_factor() {
        assert_eq!(
            to_wgpu_blend_factor(HgiBlendFactor::SrcAlpha),
            wgpu::BlendFactor::SrcAlpha
        );
    }

    #[test]
    fn test_compare_fn() {
        assert_eq!(
            to_wgpu_compare_fn(HgiCompareFunction::Less),
            wgpu::CompareFunction::Less
        );
    }

    #[test]
    fn test_stencil_op() {
        assert_eq!(
            to_wgpu_stencil_op(HgiStencilOp::Replace),
            wgpu::StencilOperation::Replace
        );
    }

    #[test]
    fn test_color_writes() {
        let mask = HgiColorMask::RED | HgiColorMask::GREEN;
        let writes = to_wgpu_color_writes(mask);
        assert!(writes.contains(wgpu::ColorWrites::RED));
        assert!(writes.contains(wgpu::ColorWrites::GREEN));
        assert!(!writes.contains(wgpu::ColorWrites::BLUE));
    }

    #[test]
    fn test_shader_stages() {
        let stage = HgiShaderStage::VERTEX | HgiShaderStage::FRAGMENT;
        let wgpu_stages = to_wgpu_shader_stages(stage);
        assert!(wgpu_stages.contains(wgpu::ShaderStages::VERTEX));
        assert!(wgpu_stages.contains(wgpu::ShaderStages::FRAGMENT));
    }
}
