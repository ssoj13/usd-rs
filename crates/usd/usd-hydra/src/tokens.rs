//! UsdHydra tokens.
//!
//! Static tokens for Hydra integration schemas.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdHydra/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// Token definitions for UsdHydra schemas.
pub struct UsdHydraTokensType {
    /// "black" - Wrap mode returning black outside texture bounds
    pub black: Token,
    /// "clamp" - Wrap mode clamping texture coordinates to [0,1]
    pub clamp: Token,
    /// "displayLook:bxdf" - Deprecated relationship for surface shader
    pub display_look_bxdf: Token,
    /// "faceIndex" - PtexTexture shader input
    pub face_index: Token,
    /// "faceOffset" - PtexTexture shader input
    pub face_offset: Token,
    /// "frame" - Texture shader input
    pub frame: Token,
    /// "HwPrimvar_1" - Primvar shader ID
    pub hw_primvar_1: Token,
    /// "HwPtexTexture_1" - PtexTexture shader ID
    pub hw_ptex_texture_1: Token,
    /// "HwUvTexture_1" - UvTexture shader ID
    pub hw_uv_texture_1: Token,
    /// "hydraGenerativeProcedural" - Default procedural system
    pub hydra_generative_procedural: Token,
    /// "inputs:file" - Texture file input
    pub info_filename: Token,
    /// "inputs:varname" - Variable name input
    pub info_varname: Token,
    /// "linear" - Linear filtering
    pub linear: Token,
    /// "linearMipmapLinear" - Trilinear filtering
    pub linear_mipmap_linear: Token,
    /// "linearMipmapNearest" - Linear/nearest mipmap filtering
    pub linear_mipmap_nearest: Token,
    /// "magFilter" - Magnification filter input
    pub mag_filter: Token,
    /// "minFilter" - Minification filter input
    pub min_filter: Token,
    /// "mirror" - Mirror wrap mode
    pub mirror: Token,
    /// "nearest" - Nearest-neighbor filtering
    pub nearest: Token,
    /// "nearestMipmapLinear" - Nearest/linear mipmap filtering
    pub nearest_mipmap_linear: Token,
    /// "nearestMipmapNearest" - Nearest/nearest mipmap filtering
    pub nearest_mipmap_nearest: Token,
    /// "primvars:hdGp:proceduralType" - Procedural type primvar
    pub primvars_hd_gp_procedural_type: Token,
    /// "proceduralSystem" - Procedural system attribute
    pub procedural_system: Token,
    /// "repeat" - Repeat wrap mode
    pub repeat: Token,
    /// "textureMemory" - Texture memory input
    pub texture_memory: Token,
    /// "useMetadata" - Use texture file metadata for wrap mode
    pub use_metadata: Token,
    /// "uv" - UV coordinate input
    pub uv: Token,
    /// "wrapS" - S-axis wrap mode input
    pub wrap_s: Token,
    /// "wrapT" - T-axis wrap mode input
    pub wrap_t: Token,
    /// "HydraGenerativeProceduralAPI" - Schema identifier
    pub hydra_generative_procedural_api: Token,
}

impl UsdHydraTokensType {
    /// Create token instances.
    fn new() -> Self {
        Self {
            black: Token::new("black"),
            clamp: Token::new("clamp"),
            display_look_bxdf: Token::new("displayLook:bxdf"),
            face_index: Token::new("faceIndex"),
            face_offset: Token::new("faceOffset"),
            frame: Token::new("frame"),
            hw_primvar_1: Token::new("HwPrimvar_1"),
            hw_ptex_texture_1: Token::new("HwPtexTexture_1"),
            hw_uv_texture_1: Token::new("HwUvTexture_1"),
            hydra_generative_procedural: Token::new("hydraGenerativeProcedural"),
            info_filename: Token::new("inputs:file"),
            info_varname: Token::new("inputs:varname"),
            linear: Token::new("linear"),
            linear_mipmap_linear: Token::new("linearMipmapLinear"),
            linear_mipmap_nearest: Token::new("linearMipmapNearest"),
            mag_filter: Token::new("magFilter"),
            min_filter: Token::new("minFilter"),
            mirror: Token::new("mirror"),
            nearest: Token::new("nearest"),
            nearest_mipmap_linear: Token::new("nearestMipmapLinear"),
            nearest_mipmap_nearest: Token::new("nearestMipmapNearest"),
            primvars_hd_gp_procedural_type: Token::new("primvars:hdGp:proceduralType"),
            procedural_system: Token::new("proceduralSystem"),
            repeat: Token::new("repeat"),
            texture_memory: Token::new("textureMemory"),
            use_metadata: Token::new("useMetadata"),
            uv: Token::new("uv"),
            wrap_s: Token::new("wrapS"),
            wrap_t: Token::new("wrapT"),
            hydra_generative_procedural_api: Token::new("HydraGenerativeProceduralAPI"),
        }
    }

    /// Get all tokens as a vector.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.black.clone(),
            self.clamp.clone(),
            self.display_look_bxdf.clone(),
            self.face_index.clone(),
            self.face_offset.clone(),
            self.frame.clone(),
            self.hw_primvar_1.clone(),
            self.hw_ptex_texture_1.clone(),
            self.hw_uv_texture_1.clone(),
            self.hydra_generative_procedural.clone(),
            self.info_filename.clone(),
            self.info_varname.clone(),
            self.linear.clone(),
            self.linear_mipmap_linear.clone(),
            self.linear_mipmap_nearest.clone(),
            self.mag_filter.clone(),
            self.min_filter.clone(),
            self.mirror.clone(),
            self.nearest.clone(),
            self.nearest_mipmap_linear.clone(),
            self.nearest_mipmap_nearest.clone(),
            self.primvars_hd_gp_procedural_type.clone(),
            self.procedural_system.clone(),
            self.repeat.clone(),
            self.texture_memory.clone(),
            self.use_metadata.clone(),
            self.uv.clone(),
            self.wrap_s.clone(),
            self.wrap_t.clone(),
            self.hydra_generative_procedural_api.clone(),
        ]
    }
}

/// Global static tokens instance.
pub static USD_HYDRA_TOKENS: LazyLock<UsdHydraTokensType> = LazyLock::new(UsdHydraTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(USD_HYDRA_TOKENS.black.as_str(), "black");
        assert_eq!(USD_HYDRA_TOKENS.repeat.as_str(), "repeat");
        assert_eq!(
            USD_HYDRA_TOKENS.hydra_generative_procedural_api.as_str(),
            "HydraGenerativeProceduralAPI"
        );
    }
}
