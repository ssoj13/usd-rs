//! HdSkinningSettings - Skinning configuration.
//!
//! Port of pxr/imaging/hd/skinningSettings.h/cpp
//!
//! Controls whether skinning is deferred to the renderer (primvars) or
//! uses extComputations. When deferred, UsdImaging relocates skelBinding
//! to primvars for instance aggregation.

use std::env;
use usd_tf::Token;

/// Check if HD_ENABLE_DEFERRED_SKINNING env is set (case-insensitive).
///
/// Port of HdSkinningSettings::IsSkinningDeferred().
/// When true, skinning inputs are emitted as primvars and deferred to
/// the renderer. UsdImaging uses DataSourceRelocatingSceneIndex to move
/// skelBinding:animationSource to primvars:skel:animationSource for
/// native instance aggregation.
#[inline]
pub fn is_skinning_deferred() -> bool {
    env::var_os("HD_ENABLE_DEFERRED_SKINNING")
        .filter(|v| !v.is_empty())
        .and_then(|v| v.into_string().ok())
        .map(|s| matches!(s.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

// HdSkinningInputTokens (HD_SKINNING_INPUT_TOKENS from pxr/imaging/hd/tokens.h)
/// Skinng xforms (4×4 matrices per joint).
pub fn skinning_xforms_token() -> Token {
    Token::new("hydra:skinningXforms")
}
/// Skinng dual quaternions (2×Vec4f per joint).
pub fn skinning_dual_quats_token() -> Token {
    Token::new("hydra:skinningDualQuats")
}
/// Skinng scale xforms (3×3, excluded from GetSkinningInputNames per HYD-3533).
pub fn skinning_scale_xforms_token() -> Token {
    Token::new("hydra:skinningScaleXforms")
}
/// Blend shape weights.
pub fn blend_shape_weights_token() -> Token {
    Token::new("hydra:blendShapeWeights")
}
/// Skeleton local to common space.
pub fn skel_local_to_common_space_token() -> Token {
    Token::new("hydra:skelLocalToWorld")
}
/// Common space to prim local.
pub fn common_space_to_prim_local_token() -> Token {
    Token::new("hydra:primWorldToLocal")
}
/// Blend shape offsets.
pub fn blend_shape_offsets_token() -> Token {
    Token::new("hydra:blendShapeOffsets")
}
/// Blend shape offset ranges.
pub fn blend_shape_offset_ranges_token() -> Token {
    Token::new("hydra:blendShapeOffsetRanges")
}
/// Num blend shape offset ranges.
pub fn num_blend_shape_offset_ranges_token() -> Token {
    Token::new("hydra:numBlendShapeOffsetRanges")
}
/// Has constant influences.
pub fn has_constant_influences_token() -> Token {
    Token::new("hydra:hasConstantInfluences")
}
/// Num influences per component.
pub fn num_influences_per_component_token() -> Token {
    Token::new("hydra:numInfluencesPerComponent")
}
/// Influences (interleaved joint indices and weights).
pub fn influences_token() -> Token {
    Token::new("hydra:influences")
}
/// Num skinning method (0=LBS, 1=DQS).
pub fn num_skinning_method_token() -> Token {
    Token::new("hydra:numSkinningMethod")
}
/// Num joints.
pub fn num_joints_token() -> Token {
    Token::new("hydra:numJoints")
}
/// Num blend shape weights.
pub fn num_blend_shape_weights_token() -> Token {
    Token::new("hydra:numBlendShapeWeights")
}

// HdSkinningSkelInputTokens (HD_SKINNING_SKEL_INPUT_TOKENS)
/// Geom bind transform.
pub fn geom_bind_transform_token() -> Token {
    Token::new("skel:geomBindTransform")
}

/// Get skinning input names for vertex shader codepath.
///
/// Port of HdSkinningSettings::GetSkinningInputNames().
/// Returns HdSkinningInputTokens (excluding skinningScaleXforms per HYD-3533)
/// plus HdSkinningSkelInputTokens.
pub fn get_skinning_input_names() -> Vec<Token> {
    let mut names = vec![
        skinning_xforms_token(),
        skinning_dual_quats_token(),
        // skinningScaleXforms excluded per HYD-3533
        blend_shape_weights_token(),
        skel_local_to_common_space_token(),
        common_space_to_prim_local_token(),
        blend_shape_offsets_token(),
        blend_shape_offset_ranges_token(),
        num_blend_shape_offset_ranges_token(),
        has_constant_influences_token(),
        num_influences_per_component_token(),
        influences_token(),
        num_skinning_method_token(),
        num_joints_token(),
        num_blend_shape_weights_token(),
    ];
    names.push(geom_bind_transform_token());
    names
}
