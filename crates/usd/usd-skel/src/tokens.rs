//! USD Skel tokens - commonly used string tokens for usdSkel module.
//!
//! Port of pxr/usd/usdSkel/tokens.h/cpp

use std::sync::OnceLock;
use usd_tf::Token;

/// USD Skel module tokens.
pub struct UsdSkelTokens {
    /// "bindTransforms" - UsdSkelSkeleton bind transforms attribute
    pub bind_transforms: Token,
    /// "blendShapes" - UsdSkelAnimation blend shapes attribute
    pub blend_shapes: Token,
    /// "blendShapeWeights" - UsdSkelAnimation blend shape weights attribute
    pub blend_shape_weights: Token,
    /// "classicLinear" - Fallback value for UsdSkelBindingAPI::GetSkinningMethodAttr()
    pub classic_linear: Token,
    /// "dualQuaternion" - Possible value for UsdSkelBindingAPI::GetSkinningMethodAttr()
    pub dual_quaternion: Token,
    /// "jointNames" - UsdSkelSkeleton joint names attribute
    pub joint_names: Token,
    /// "joints" - UsdSkelSkeleton, UsdSkelAnimation joints attribute
    pub joints: Token,
    /// "normalOffsets" - UsdSkelBlendShape normal offsets attribute
    pub normal_offsets: Token,
    /// "offsets" - UsdSkelBlendShape offsets attribute
    pub offsets: Token,
    /// "pointIndices" - UsdSkelBlendShape point indices attribute
    pub point_indices: Token,
    /// "primvars:skel:geomBindTransform" - UsdSkelBindingAPI geom bind transform primvar
    pub primvars_skel_geom_bind_transform: Token,
    /// "primvars:skel:jointIndices" - UsdSkelBindingAPI joint indices primvar
    pub primvars_skel_joint_indices: Token,
    /// "primvars:skel:jointWeights" - UsdSkelBindingAPI joint weights primvar
    pub primvars_skel_joint_weights: Token,
    /// "primvars:skel:skinningMethod" - UsdSkelBindingAPI skinning method primvar
    pub primvars_skel_skinning_method: Token,
    /// "restTransforms" - UsdSkelSkeleton rest transforms attribute
    pub rest_transforms: Token,
    /// "rotations" - UsdSkelAnimation rotations attribute
    pub rotations: Token,
    /// "scales" - UsdSkelAnimation scales attribute
    pub scales: Token,
    /// "skel:animationSource" - UsdSkelBindingAPI animation source relationship
    pub skel_animation_source: Token,
    /// "skel:blendShapes" - UsdSkelBindingAPI blend shapes relationship
    pub skel_blend_shapes: Token,
    /// "skel:blendShapeTargets" - UsdSkelBindingAPI blend shape targets relationship
    pub skel_blend_shape_targets: Token,
    /// "skel:joints" - UsdSkelBindingAPI joints relationship
    pub skel_joints: Token,
    /// "skel:skeleton" - UsdSkelBindingAPI skeleton relationship
    pub skel_skeleton: Token,
    /// "translations" - UsdSkelAnimation translations attribute
    pub translations: Token,
    /// "weight" - UsdSkelInbetweenShape weight attribute
    pub weight: Token,
    /// "BlendShape" - Schema identifier and family for UsdSkelBlendShape
    pub blend_shape: Token,
    /// "SkelAnimation" - Schema identifier and family for UsdSkelAnimation
    pub skel_animation: Token,
    /// "SkelBindingAPI" - Schema identifier and family for UsdSkelBindingAPI
    pub skel_binding_api: Token,
    /// "Skeleton" - Schema identifier and family for UsdSkelSkeleton
    pub skeleton: Token,
    /// "SkelRoot" - Schema identifier and family for UsdSkelRoot
    pub skel_root: Token,
}

static TOKENS: OnceLock<UsdSkelTokens> = OnceLock::new();

impl UsdSkelTokens {
    /// Get the global tokens instance.
    pub fn get() -> &'static UsdSkelTokens {
        TOKENS.get_or_init(|| UsdSkelTokens {
            bind_transforms: Token::new("bindTransforms"),
            blend_shapes: Token::new("blendShapes"),
            blend_shape_weights: Token::new("blendShapeWeights"),
            classic_linear: Token::new("classicLinear"),
            dual_quaternion: Token::new("dualQuaternion"),
            joint_names: Token::new("jointNames"),
            joints: Token::new("joints"),
            normal_offsets: Token::new("normalOffsets"),
            offsets: Token::new("offsets"),
            point_indices: Token::new("pointIndices"),
            primvars_skel_geom_bind_transform: Token::new("primvars:skel:geomBindTransform"),
            primvars_skel_joint_indices: Token::new("primvars:skel:jointIndices"),
            primvars_skel_joint_weights: Token::new("primvars:skel:jointWeights"),
            primvars_skel_skinning_method: Token::new("primvars:skel:skinningMethod"),
            rest_transforms: Token::new("restTransforms"),
            rotations: Token::new("rotations"),
            scales: Token::new("scales"),
            skel_animation_source: Token::new("skel:animationSource"),
            skel_blend_shapes: Token::new("skel:blendShapes"),
            skel_blend_shape_targets: Token::new("skel:blendShapeTargets"),
            skel_joints: Token::new("skel:joints"),
            skel_skeleton: Token::new("skel:skeleton"),
            translations: Token::new("translations"),
            weight: Token::new("weight"),
            blend_shape: Token::new("BlendShape"),
            skel_animation: Token::new("SkelAnimation"),
            skel_binding_api: Token::new("SkelBindingAPI"),
            skeleton: Token::new("Skeleton"),
            skel_root: Token::new("SkelRoot"),
        })
    }
}

/// Get the global UsdSkelTokens instance.
///
/// Matches C++ `UsdSkelTokens`.
pub fn tokens() -> &'static UsdSkelTokens {
    UsdSkelTokens::get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        let t = tokens();
        assert_eq!(t.bind_transforms.as_str(), "bindTransforms");
        assert_eq!(t.joints.as_str(), "joints");
        assert_eq!(t.skel_skeleton.as_str(), "skel:skeleton");
        assert_eq!(t.skeleton.as_str(), "Skeleton");
    }
}
