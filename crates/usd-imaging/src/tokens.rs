//! UsdImaging tokens.

use std::sync::LazyLock;
use usd_tf::Token;

/// UsdImaging tokens for special attributes, primvars, and metadata.
#[derive(Debug, Clone)]
pub struct UsdImagingTokens;

impl UsdImagingTokens {
    /// Collection light link attribute name
    pub fn collection_light_link() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("collection:lightLink"));
        &TOKEN
    }

    /// Collection shadow link attribute name
    pub fn collection_shadow_link() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("collection:shadowLink"));
        &TOKEN
    }

    /// Config attribute prefix
    pub fn config_prefix() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("config:"));
        &TOKEN
    }

    /// Face index primvar (for ptex)
    pub fn face_index_primvar() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("faceIndexPrimvar"));
        &TOKEN
    }

    /// Face offset primvar (for ptex)
    pub fn face_offset_primvar() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("faceOffsetPrimvar"));
        &TOKEN
    }

    /// Normals primvar attribute name
    pub fn primvars_normals() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("primvars:normals"));
        &TOKEN
    }

    /// Widths primvar attribute name
    pub fn primvars_widths() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("primvars:widths"));
        &TOKEN
    }

    /// Ptex face index
    pub fn ptex_face_index() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("ptexFaceIndex"));
        &TOKEN
    }

    /// Ptex face offset
    pub fn ptex_face_offset() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("ptexFaceOffset"));
        &TOKEN
    }

    /// USD populated prim count metadata
    pub fn usd_populated_prim_count() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdPopulatedPrimCount"));
        &TOKEN
    }

    /// USD varying extent dirty bit
    pub fn usd_varying_extent() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingExtent"));
        &TOKEN
    }

    /// USD varying primvar dirty bit
    pub fn usd_varying_primvar() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingPrimvar"));
        &TOKEN
    }

    /// USD varying topology dirty bit
    pub fn usd_varying_topology() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingTopology"));
        &TOKEN
    }

    /// USD varying visibility dirty bit
    pub fn usd_varying_visibility() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingVisibility"));
        &TOKEN
    }

    /// USD varying widths dirty bit
    pub fn usd_varying_widths() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingWidths"));
        &TOKEN
    }

    /// USD varying normals dirty bit
    pub fn usd_varying_normals() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingNormals"));
        &TOKEN
    }

    /// USD varying transform dirty bit
    pub fn usd_varying_xform() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingXform"));
        &TOKEN
    }

    /// USD varying texture dirty bit
    pub fn usd_varying_texture() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("usdVaryingTexture"));
        &TOKEN
    }

    /// UV primvar
    pub fn uv_primvar() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("uvPrimvar"));
        &TOKEN
    }

    /// UsdPreviewSurface shader ID
    pub fn usd_preview_surface() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdPreviewSurface"));
        &TOKEN
    }

    /// UsdUVTexture shader ID
    pub fn usd_uv_texture() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdUVTexture"));
        &TOKEN
    }

    /// UsdPrimvarReader_float shader ID
    pub fn usd_primvar_reader_float() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdPrimvarReader_float"));
        &TOKEN
    }

    /// UsdPrimvarReader_float2 shader ID
    pub fn usd_primvar_reader_float2() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdPrimvarReader_float2"));
        &TOKEN
    }

    /// UsdPrimvarReader_float3 shader ID
    pub fn usd_primvar_reader_float3() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdPrimvarReader_float3"));
        &TOKEN
    }

    /// UsdPrimvarReader_float4 shader ID
    pub fn usd_primvar_reader_float4() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdPrimvarReader_float4"));
        &TOKEN
    }

    /// UsdPrimvarReader_int shader ID
    pub fn usd_primvar_reader_int() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdPrimvarReader_int"));
        &TOKEN
    }

    /// UsdTransform2d shader ID
    pub fn usd_transform2d() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("UsdTransform2d"));
        &TOKEN
    }

    /// Stage scene index repopulate notice token
    pub fn stage_scene_index_repopulate() -> &'static Token {
        static TOKEN: LazyLock<Token> =
            LazyLock::new(|| Token::new("__usdStageSceneIndexRepopulate"));
        &TOKEN
    }

    /// Stage scene index includeUnloadedPrims input arg (UsdImagingStageSceneIndexTokens)
    pub fn stage_scene_index_include_unloaded_prims() -> &'static Token {
        static TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("includeUnloadedPrims"));
        &TOKEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(
            UsdImagingTokens::collection_light_link().as_str(),
            "collection:lightLink"
        );
        assert_eq!(
            UsdImagingTokens::primvars_normals().as_str(),
            "primvars:normals"
        );
        assert_eq!(
            UsdImagingTokens::usd_preview_surface().as_str(),
            "UsdPreviewSurface"
        );
    }

    #[test]
    fn test_token_identity() {
        // Same token should return same reference
        let t1 = UsdImagingTokens::usd_varying_extent();
        let t2 = UsdImagingTokens::usd_varying_extent();
        assert!(std::ptr::eq(t1, t2));
    }
}
