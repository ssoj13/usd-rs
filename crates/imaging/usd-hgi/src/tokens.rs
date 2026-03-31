//! Token definitions for Hydra Graphics Interface (HGI).
//!
//! HGI provides a hardware-agnostic abstraction layer for low-level graphics APIs
//! (OpenGL, Metal, Vulkan) used by Hydra rendering delegates.
//!
//! This module defines tokens for:
//! - Graphics driver identification
//! - Shader keyword mappings for built-in variables

use once_cell::sync::Lazy;
use usd_tf::Token;

// ============================================================================
// Graphics Driver Tokens
// ============================================================================

/// Task driver token identifier.
///
/// Used to identify task-based GPU compute drivers in the HGI subsystem.
pub static TASK_DRIVER: Lazy<Token> = Lazy::new(|| Token::new("taskDriver"));

/// Render driver token identifier.
///
/// Used to identify graphics rendering drivers in the HGI subsystem.
pub static RENDER_DRIVER: Lazy<Token> = Lazy::new(|| Token::new("renderDriver"));

/// OpenGL graphics API token.
///
/// Identifies the OpenGL/OpenGL ES backend implementation.
pub static OPENGL: Lazy<Token> = Lazy::new(|| Token::new("OpenGL"));

/// Metal graphics API token.
///
/// Identifies the Apple Metal backend implementation (macOS/iOS).
pub static METAL: Lazy<Token> = Lazy::new(|| Token::new("Metal"));

/// Vulkan graphics API token.
///
/// Identifies the Vulkan backend implementation.
pub static VULKAN: Lazy<Token> = Lazy::new(|| Token::new("Vulkan"));

// ============================================================================
// Shader Keyword Tokens (Built-in Variables)
// ============================================================================

/// Vertex position in clip space.
///
/// Maps to:
/// - GLSL: `gl_Position`
/// - HLSL: `SV_Position`
/// - Metal: `[[position]]`
pub static HD_POSITION: Lazy<Token> = Lazy::new(|| Token::new("hdPosition"));

/// Point sprite coordinate.
///
/// Maps to:
/// - GLSL: `gl_PointCoord`
/// - HLSL: `SV_Position` (for point sprites)
/// - Metal: `[[point_coord]]`
pub static HD_POINT_COORD: Lazy<Token> = Lazy::new(|| Token::new("hdPointCoord"));

/// Clip distance for user-defined clipping planes.
///
/// Maps to:
/// - GLSL: `gl_ClipDistance`
/// - HLSL: `SV_ClipDistance`
/// - Metal: `[[clip_distance]]`
pub static HD_CLIP_DISTANCE: Lazy<Token> = Lazy::new(|| Token::new("hdClipDistance"));

/// Cull distance for user-defined culling.
///
/// Maps to:
/// - GLSL: `gl_CullDistance`
/// - HLSL: `SV_CullDistance`
/// - Metal: `[[cull_distance]]`
pub static HD_CULL_DISTANCE: Lazy<Token> = Lazy::new(|| Token::new("hdCullDistance"));

/// Vertex identifier within a draw call.
///
/// Maps to:
/// - GLSL: `gl_VertexID`
/// - HLSL: `SV_VertexID`
/// - Metal: `[[vertex_id]]`
pub static HD_VERTEX_ID: Lazy<Token> = Lazy::new(|| Token::new("hdVertexID"));

/// Instance identifier for instanced rendering.
///
/// Maps to:
/// - GLSL: `gl_InstanceID`
/// - HLSL: `SV_InstanceID`
/// - Metal: `[[instance_id]]`
pub static HD_INSTANCE_ID: Lazy<Token> = Lazy::new(|| Token::new("hdInstanceID"));

/// Primitive identifier within a draw call.
///
/// Maps to:
/// - GLSL: `gl_PrimitiveID`
/// - HLSL: `SV_PrimitiveID`
/// - Metal: `[[primitive_id]]`
pub static HD_PRIMITIVE_ID: Lazy<Token> = Lazy::new(|| Token::new("hdPrimitiveID"));

/// Sample identifier for multi-sampling.
///
/// Maps to:
/// - GLSL: `gl_SampleID`
/// - HLSL: `SV_SampleIndex`
/// - Metal: `[[sample_id]]`
pub static HD_SAMPLE_ID: Lazy<Token> = Lazy::new(|| Token::new("hdSampleID"));

/// Sample position for multi-sampling.
///
/// Maps to:
/// - GLSL: `gl_SamplePosition`
/// - HLSL: `GetSamplePosition()`
/// - Metal: `[[sample_position]]`
pub static HD_SAMPLE_POSITION: Lazy<Token> = Lazy::new(|| Token::new("hdSamplePosition"));

/// Fragment coordinate in window space.
///
/// Maps to:
/// - GLSL: `gl_FragCoord`
/// - HLSL: `SV_Position`
/// - Metal: `[[position]]`
pub static HD_FRAG_COORD: Lazy<Token> = Lazy::new(|| Token::new("hdFragCoord"));

/// Fragment front-facing flag.
///
/// Maps to:
/// - GLSL: `gl_FrontFacing`
/// - HLSL: `SV_IsFrontFace`
/// - Metal: `[[front_facing]]`
pub static HD_FRONT_FACING: Lazy<Token> = Lazy::new(|| Token::new("hdFrontFacing"));

/// Layer index for layered rendering (geometry shader output).
///
/// Maps to:
/// - GLSL: `gl_Layer`
/// - HLSL: `SV_RenderTargetArrayIndex`
/// - Metal: `[[render_target_array_index]]`
pub static HD_LAYER: Lazy<Token> = Lazy::new(|| Token::new("hdLayer"));

/// Base vertex offset for indexed draws.
///
/// Maps to:
/// - GLSL: `gl_BaseVertex`
/// - HLSL: `SV_StartVertexLocation`
/// - Metal: `[[base_vertex]]`
pub static HD_BASE_VERTEX: Lazy<Token> = Lazy::new(|| Token::new("hdBaseVertex"));

/// Base instance offset for instanced draws.
///
/// Maps to:
/// - GLSL: `gl_BaseInstance`
/// - HLSL: `SV_StartInstanceLocation`
/// - Metal: `[[base_instance]]`
pub static HD_BASE_INSTANCE: Lazy<Token> = Lazy::new(|| Token::new("hdBaseInstance"));

/// Viewport index for multi-viewport rendering.
///
/// Maps to:
/// - GLSL: `gl_ViewportIndex`
/// - HLSL: `SV_ViewportArrayIndex`
/// - Metal: `[[viewport_array_index]]`
pub static HD_VIEWPORT_INDEX: Lazy<Token> = Lazy::new(|| Token::new("hdViewportIndex"));

/// Tessellation patch position coordinate.
///
/// Maps to:
/// - GLSL: `gl_TessCoord`
/// - HLSL: `SV_DomainLocation`
/// - Metal: `[[position_in_patch]]`
pub static HD_POSITION_IN_PATCH: Lazy<Token> = Lazy::new(|| Token::new("hdPositionInPatch"));

/// Tessellation patch identifier.
///
/// Maps to:
/// - GLSL: `gl_PatchVerticesIn` / `gl_InvocationID`
/// - HLSL: `SV_PrimitiveID`
/// - Metal: `[[patch_id]]`
pub static HD_PATCH_ID: Lazy<Token> = Lazy::new(|| Token::new("hdPatchID"));

/// Global invocation identifier for compute shaders.
///
/// Maps to:
/// - GLSL: `gl_GlobalInvocationID`
/// - HLSL: `SV_DispatchThreadID`
/// - Metal: `[[thread_position_in_grid]]`
pub static HD_GLOBAL_INVOCATION_ID: Lazy<Token> = Lazy::new(|| Token::new("hdGlobalInvocationID"));

/// Barycentric coordinates without perspective correction.
///
/// Maps to:
/// - GLSL: `gl_BaryCoordNoPerspNV` (extension)
/// - HLSL: `SV_Barycentrics` with `noperspective`
/// - Metal: `[[barycentric_coord]]` with `flat`
pub static HD_BARY_COORD_NO_PERSP: Lazy<Token> = Lazy::new(|| Token::new("hdBaryCoordNoPersp"));

/// Input sample coverage mask for fragment shaders.
///
/// Maps to:
/// - GLSL: `gl_SampleMaskIn`
/// - HLSL: `SV_Coverage` (input)
/// - Metal: `[[sample_mask]]` (input)
pub static HD_SAMPLE_MASK_IN: Lazy<Token> = Lazy::new(|| Token::new("hdSampleMaskIn"));

/// Output sample coverage mask for fragment shaders.
///
/// Maps to:
/// - GLSL: `gl_SampleMask`
/// - HLSL: `SV_Coverage` (output)
/// - Metal: `[[sample_mask]]` (output)
pub static HD_SAMPLE_MASK: Lazy<Token> = Lazy::new(|| Token::new("hdSampleMask"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_tokens() {
        assert_eq!(OPENGL.as_str(), "OpenGL");
        assert_eq!(METAL.as_str(), "Metal");
        assert_eq!(VULKAN.as_str(), "Vulkan");
    }

    #[test]
    fn test_shader_tokens() {
        assert_eq!(HD_POSITION.as_str(), "hdPosition");
        assert_eq!(HD_VERTEX_ID.as_str(), "hdVertexID");
        assert_eq!(HD_FRAG_COORD.as_str(), "hdFragCoord");
    }
}
