//! Tokens for UsdSkelImaging.
//!
//! Port of pxr/usdImaging/usdSkelImaging/tokens.h
//!
//! This module defines token collections used throughout the UsdSkelImaging
//! module for ext computation types, names, inputs, and outputs related to
//! skeletal skinning and deformation.

use std::sync::LazyLock;
use usd_tf::Token;

// ============================================================================
// Ext Computation Type Tokens
// ============================================================================

/// Ext computation type tokens for skeletal deformation.
pub static EXT_COMPUTATION_TYPE_TOKENS: LazyLock<ExtComputationTypeTokens> =
    LazyLock::new(ExtComputationTypeTokens::new);

/// Tokens for ext computation types in skeletal imaging.
///
/// These tokens identify the type of GPU-accelerated computations used for
/// skeletal animation (skinning, blend shapes). Corresponds to
/// `UsdSkelImagingTokens` in `pxr/usdImaging/usdSkelImaging/tokens.h`.
#[derive(Debug, Clone)]
pub struct ExtComputationTypeTokens {
    /// Points computation - deforms vertex positions
    pub points: Token,
    /// Normals computation - deforms vertex normals
    pub normals: Token,
}

impl ExtComputationTypeTokens {
    fn new() -> Self {
        Self {
            points: Token::new("points"),
            normals: Token::new("normals"),
        }
    }
}

// ============================================================================
// Prim Type Tokens
// ============================================================================

/// Prim type tokens for skeletal prims.
pub static PRIM_TYPE_TOKENS: LazyLock<PrimTypeTokens> = LazyLock::new(PrimTypeTokens::new);

/// Tokens for skeletal prim types.
///
/// Identifies USD prim types related to skeletal animation: skeletons,
/// skeletal animations, and blend shapes. Corresponds to prim type tokens
/// in `pxr/usdImaging/usdSkelImaging/tokens.h`.
#[derive(Debug, Clone)]
pub struct PrimTypeTokens {
    /// Skeleton prim type
    pub skeleton: Token,
    /// Skeletal animation prim type
    pub skel_animation: Token,
    /// Blend shape prim type
    pub skel_blend_shape: Token,
}

impl PrimTypeTokens {
    fn new() -> Self {
        Self {
            skeleton: Token::new("skeleton"),
            skel_animation: Token::new("skelAnimation"),
            skel_blend_shape: Token::new("skelBlendShape"),
        }
    }
}

// ============================================================================
// Ext Computation Name Tokens
// ============================================================================

/// Ext computation name tokens.
pub static EXT_COMPUTATION_NAME_TOKENS: LazyLock<ExtComputationNameTokens> =
    LazyLock::new(ExtComputationNameTokens::new);

/// Tokens for ext computation names.
///
/// Names for GPU computations that perform skinning and blend shape
/// deformation. Includes both aggregator computations (data preparation)
/// and main computations (actual deformation). Corresponds to computation
/// name tokens in `pxr/usdImaging/usdSkelImaging/tokens.h`.
#[derive(Debug, Clone)]
pub struct ExtComputationNameTokens {
    /// Points aggregator computation name
    pub points_aggregator_computation: Token,
    /// Points computation name
    pub points_computation: Token,
    /// Normals aggregator computation name
    pub normals_aggregator_computation: Token,
    /// Normals computation name
    pub normals_computation: Token,
}

impl ExtComputationNameTokens {
    fn new() -> Self {
        Self {
            points_aggregator_computation: Token::new("skinningPointsInputAggregatorComputation"),
            points_computation: Token::new("skinningPointsComputation"),
            normals_aggregator_computation: Token::new("skinningNormalsInputAggregatorComputation"),
            normals_computation: Token::new("skinningNormalsComputation"),
        }
    }
}

// ============================================================================
// Aggregator Computation Input Name Tokens
// ============================================================================

/// Aggregator computation input name tokens.
pub static EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS: LazyLock<ExtAggregatorComputationInputTokens> =
    LazyLock::new(ExtAggregatorComputationInputTokens::new);

/// Tokens for aggregator computation input parameters.
///
/// Input parameters for the aggregator stage of GPU skinning, which prepares
/// static geometry data (rest pose, influences, bind transforms) and blend
/// shape data for the deformation computation. These inputs are typically
/// constant per mesh. Corresponds to input tokens in
/// `pxr/usdImaging/usdSkelImaging/tokens.h`.
#[derive(Debug, Clone)]
pub struct ExtAggregatorComputationInputTokens {
    /// Rest position points
    pub rest_points: Token,
    /// Geom bind transform
    pub geom_bind_xform: Token,
    /// Joint influences (weights and indices)
    pub influences: Token,
    /// Number of influences per component
    pub num_influences_per_component: Token,
    /// Whether influences are constant
    pub has_constant_influences: Token,
    /// Blend shape offset vectors
    pub blend_shape_offsets: Token,
    /// Blend shape offset ranges
    pub blend_shape_offset_ranges: Token,
    /// Number of blend shape offset ranges
    pub num_blend_shape_offset_ranges: Token,
    /// Rest normals
    pub rest_normals: Token,
    /// Face vertex indices (for computing face-varying normals)
    pub face_vertex_indices: Token,
    /// Whether normals are face-varying
    pub has_face_varying_normals: Token,
}

impl ExtAggregatorComputationInputTokens {
    fn new() -> Self {
        Self {
            rest_points: Token::new("restPoints"),
            geom_bind_xform: Token::new("geomBindXform"),
            influences: Token::new("influences"),
            num_influences_per_component: Token::new("numInfluencesPerComponent"),
            has_constant_influences: Token::new("hasConstantInfluences"),
            blend_shape_offsets: Token::new("blendShapeOffsets"),
            blend_shape_offset_ranges: Token::new("blendShapeOffsetRanges"),
            num_blend_shape_offset_ranges: Token::new("numBlendShapeOffsetRanges"),
            rest_normals: Token::new("restNormals"),
            face_vertex_indices: Token::new("faceVertexIndices"),
            has_face_varying_normals: Token::new("hasFaceVaryingNormals"),
        }
    }
}

// ============================================================================
// Ext Computation Input Name Tokens
// ============================================================================

/// Ext computation input name tokens.
pub static EXT_COMPUTATION_INPUT_TOKENS: LazyLock<ExtComputationInputTokens> =
    LazyLock::new(ExtComputationInputTokens::new);

/// Tokens for ext computation input parameters.
///
/// Input parameters for the main skinning computation, which receives
/// time-varying animation data (blend shape weights, joint transforms) and
/// coordinate space transforms. These inputs change per frame. Corresponds
/// to input tokens in `pxr/usdImaging/usdSkelImaging/tokens.h`.
#[derive(Debug, Clone)]
pub struct ExtComputationInputTokens {
    /// Blend shape weights (animated)
    pub blend_shape_weights: Token,
    /// Skinning transforms (joint matrices)
    pub skinning_xforms: Token,
    /// Skinning scale transforms
    pub skinning_scale_xforms: Token,
    /// Skinning dual quaternions
    pub skinning_dual_quats: Token,
    /// Skeleton local to common space transform
    pub skel_local_to_common_space: Token,
    /// Common space to prim local transform
    pub common_space_to_prim_local: Token,
}

impl ExtComputationInputTokens {
    fn new() -> Self {
        Self {
            blend_shape_weights: Token::new("blendShapeWeights"),
            skinning_xforms: Token::new("skinningXforms"),
            skinning_scale_xforms: Token::new("skinningScaleXforms"),
            skinning_dual_quats: Token::new("skinningDualQuats"),
            skel_local_to_common_space: Token::new("skelLocalToWorld"),
            common_space_to_prim_local: Token::new("primWorldToLocal"),
        }
    }
}

// ============================================================================
// Legacy Ext Computation Input Name Tokens
// ============================================================================

/// Legacy ext computation input name tokens.
pub static EXT_COMPUTATION_LEGACY_INPUT_TOKENS: LazyLock<ExtComputationLegacyInputTokens> =
    LazyLock::new(ExtComputationLegacyInputTokens::new);

/// Legacy tokens for ext computation inputs.
///
/// Backward compatibility tokens for older USD versions that used different
/// naming conventions for coordinate space transforms. Corresponds to legacy
/// tokens in `pxr/usdImaging/usdSkelImaging/tokens.h`.
#[derive(Debug, Clone)]
pub struct ExtComputationLegacyInputTokens {
    /// Legacy name for skeleton local to common space
    pub skel_local_to_world: Token,
    /// Legacy name for common space to prim local
    pub prim_world_to_local: Token,
}

impl ExtComputationLegacyInputTokens {
    fn new() -> Self {
        Self {
            skel_local_to_world: Token::new("skelLocalToWorld"),
            prim_world_to_local: Token::new("primWorldToLocal"),
        }
    }
}

// ============================================================================
// Ext Computation Output Name Tokens
// ============================================================================

/// Ext computation output name tokens.
pub static EXT_COMPUTATION_OUTPUT_TOKENS: LazyLock<ExtComputationOutputTokens> =
    LazyLock::new(ExtComputationOutputTokens::new);

/// Tokens for ext computation output parameters.
///
/// Output parameters from GPU skinning computations, providing the final
/// deformed vertex positions and normals after applying skeletal animation
/// and blend shapes. Corresponds to output tokens in
/// `pxr/usdImaging/usdSkelImaging/tokens.h`.
#[derive(Debug, Clone)]
pub struct ExtComputationOutputTokens {
    /// Skinned points output
    pub skinned_points: Token,
    /// Skinned normals output
    pub skinned_normals: Token,
}

impl ExtComputationOutputTokens {
    fn new() -> Self {
        Self {
            skinned_points: Token::new("skinnedPoints"),
            skinned_normals: Token::new("skinnedNormals"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ext_computation_type_tokens() {
        assert_eq!(EXT_COMPUTATION_TYPE_TOKENS.points.as_str(), "points");
        assert_eq!(EXT_COMPUTATION_TYPE_TOKENS.normals.as_str(), "normals");
    }

    #[test]
    fn test_prim_type_tokens() {
        assert_eq!(PRIM_TYPE_TOKENS.skeleton.as_str(), "skeleton");
        assert_eq!(PRIM_TYPE_TOKENS.skel_animation.as_str(), "skelAnimation");
        assert_eq!(PRIM_TYPE_TOKENS.skel_blend_shape.as_str(), "skelBlendShape");
    }

    #[test]
    fn test_ext_computation_name_tokens() {
        assert_eq!(
            EXT_COMPUTATION_NAME_TOKENS
                .points_aggregator_computation
                .as_str(),
            "skinningPointsInputAggregatorComputation"
        );
        assert_eq!(
            EXT_COMPUTATION_NAME_TOKENS.points_computation.as_str(),
            "skinningPointsComputation"
        );
    }

    #[test]
    fn test_ext_aggregator_input_tokens() {
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_points.as_str(),
            "restPoints"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.influences.as_str(),
            "influences"
        );
    }

    #[test]
    fn test_ext_computation_input_tokens() {
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights.as_str(),
            "blendShapeWeights"
        );
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms.as_str(),
            "skinningXforms"
        );
    }

    #[test]
    fn test_ext_computation_output_tokens() {
        assert_eq!(
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_points.as_str(),
            "skinnedPoints"
        );
        assert_eq!(
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_normals.as_str(),
            "skinnedNormals"
        );
    }

    #[test]
    fn test_legacy_tokens() {
        assert_eq!(
            EXT_COMPUTATION_LEGACY_INPUT_TOKENS
                .skel_local_to_world
                .as_str(),
            "skelLocalToWorld"
        );
        assert_eq!(
            EXT_COMPUTATION_LEGACY_INPUT_TOKENS
                .prim_world_to_local
                .as_str(),
            "primWorldToLocal"
        );
    }
}
