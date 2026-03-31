//! RenderMan USD Imaging tokens (kept for compatibility, not actively used).
//!
//! Port of `pxr/usdImaging/usdRiPxrImaging/tokens.h`.
//! usd-rs uses wgpu, not RenderMan. Tokens preserved so downstream code
//! referencing RenderMan prim types does not break.

use usd_tf::Token;

/// Main UsdRiPxrImaging tokens (same name as parent module - follows OpenUSD pattern).
#[allow(clippy::module_inception)]
pub mod tokens {
    use super::Token;

    /// info:source token
    pub fn info_source() -> Token {
        Token::new("info:source")
    }

    /// faceIndexPrimvar token
    pub fn face_index_primvar() -> Token {
        Token::new("faceIndexPrimvar")
    }

    /// faceOffsetPrimvar token
    pub fn face_offset_primvar() -> Token {
        Token::new("faceOffsetPrimvar")
    }

    /// primvars:normals token
    pub fn primvars_normals() -> Token {
        Token::new("primvars:normals")
    }

    /// primvars:widths token
    pub fn primvars_widths() -> Token {
        Token::new("primvars:widths")
    }

    /// ptexFaceIndex token
    pub fn ptex_face_index() -> Token {
        Token::new("ptexFaceIndex")
    }

    /// ptexFaceOffset token
    pub fn ptex_face_offset() -> Token {
        Token::new("ptexFaceOffset")
    }

    /// usdPopulatedPrimCount token
    pub fn usd_populated_prim_count() -> Token {
        Token::new("usdPopulatedPrimCount")
    }

    /// usdVaryingExtent token
    pub fn usd_varying_extent() -> Token {
        Token::new("usdVaryingExtent")
    }

    /// usdVaryingPrimvar token
    pub fn usd_varying_primvar() -> Token {
        Token::new("usdVaryingPrimvar")
    }

    /// usdVaryingTopology token
    pub fn usd_varying_topology() -> Token {
        Token::new("usdVaryingTopology")
    }

    /// usdVaryingVisibility token
    pub fn usd_varying_visibility() -> Token {
        Token::new("usdVaryingVisibility")
    }

    /// usdVaryingWidths token
    pub fn usd_varying_widths() -> Token {
        Token::new("usdVaryingWidths")
    }

    /// usdVaryingNormals token
    pub fn usd_varying_normals() -> Token {
        Token::new("usdVaryingNormals")
    }

    /// usdVaryingXform token
    pub fn usd_varying_xform() -> Token {
        Token::new("usdVaryingXform")
    }

    /// usdVaryingTexture token
    pub fn usd_varying_texture() -> Token {
        Token::new("usdVaryingTexture")
    }

    /// uvPrimvar token
    pub fn uv_primvar() -> Token {
        Token::new("uvPrimvar")
    }

    /// UsdPreviewSurface token
    pub fn usd_preview_surface() -> Token {
        Token::new("UsdPreviewSurface")
    }

    /// UsdUVTexture token
    pub fn usd_uv_texture() -> Token {
        Token::new("UsdUVTexture")
    }

    /// UsdPrimvarReader_float token
    pub fn usd_primvar_reader_float() -> Token {
        Token::new("UsdPrimvarReader_float")
    }

    /// UsdPrimvarReader_float2 token
    pub fn usd_primvar_reader_float2() -> Token {
        Token::new("UsdPrimvarReader_float2")
    }

    /// UsdPrimvarReader_float3 token
    pub fn usd_primvar_reader_float3() -> Token {
        Token::new("UsdPrimvarReader_float3")
    }

    /// UsdPrimvarReader_float4 token
    pub fn usd_primvar_reader_float4() -> Token {
        Token::new("UsdPrimvarReader_float4")
    }

    /// UsdPrimvarReader_int token
    pub fn usd_primvar_reader_int() -> Token {
        Token::new("UsdPrimvarReader_int")
    }

    /// UsdTransform2d token
    pub fn usd_transform2d() -> Token {
        Token::new("UsdTransform2d")
    }

    /// pxrBarnLightFilter token
    pub fn pxr_barn_light_filter() -> Token {
        Token::new("pxrBarnLightFilter")
    }

    /// pxrIntMultLightFilter token
    pub fn pxr_int_mult_light_filter() -> Token {
        Token::new("pxrIntMultLightFilter")
    }

    /// pxrRodLightFilter token
    pub fn pxr_rod_light_filter() -> Token {
        Token::new("pxrRodLightFilter")
    }
}

/// Prim type tokens for UsdRiPxrImaging.
pub mod prim_type_tokens {
    use super::Token;

    /// projection prim type token
    pub fn projection() -> Token {
        Token::new("projection")
    }
}
