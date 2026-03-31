//! Tests for UsdSkelImaging module.

#[cfg(test)]
mod token_tests {
    use crate::skel::tokens::*;

    #[test]
    fn test_ext_computation_type_tokens() {
        // Verify computation type tokens
        assert_eq!(EXT_COMPUTATION_TYPE_TOKENS.points.as_str(), "points");
        assert_eq!(EXT_COMPUTATION_TYPE_TOKENS.normals.as_str(), "normals");

        // Ensure tokens are unique
        assert_ne!(
            EXT_COMPUTATION_TYPE_TOKENS.points,
            EXT_COMPUTATION_TYPE_TOKENS.normals
        );
    }

    #[test]
    fn test_prim_type_tokens() {
        // Verify prim type tokens
        assert_eq!(PRIM_TYPE_TOKENS.skeleton.as_str(), "skeleton");
        assert_eq!(PRIM_TYPE_TOKENS.skel_animation.as_str(), "skelAnimation");
        assert_eq!(PRIM_TYPE_TOKENS.skel_blend_shape.as_str(), "skelBlendShape");

        // Ensure all tokens are unique
        assert_ne!(PRIM_TYPE_TOKENS.skeleton, PRIM_TYPE_TOKENS.skel_animation);
        assert_ne!(PRIM_TYPE_TOKENS.skeleton, PRIM_TYPE_TOKENS.skel_blend_shape);
        assert_ne!(
            PRIM_TYPE_TOKENS.skel_animation,
            PRIM_TYPE_TOKENS.skel_blend_shape
        );
    }

    #[test]
    fn test_ext_computation_name_tokens() {
        // Verify computation name tokens
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
        assert_eq!(
            EXT_COMPUTATION_NAME_TOKENS
                .normals_aggregator_computation
                .as_str(),
            "skinningNormalsInputAggregatorComputation"
        );
        assert_eq!(
            EXT_COMPUTATION_NAME_TOKENS.normals_computation.as_str(),
            "skinningNormalsComputation"
        );

        // Ensure all computation names are unique (critical for Hydra)
        let names = vec![
            &EXT_COMPUTATION_NAME_TOKENS.points_aggregator_computation,
            &EXT_COMPUTATION_NAME_TOKENS.points_computation,
            &EXT_COMPUTATION_NAME_TOKENS.normals_aggregator_computation,
            &EXT_COMPUTATION_NAME_TOKENS.normals_computation,
        ];

        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                assert_ne!(names[i], names[j], "Computation names must be unique");
            }
        }
    }

    #[test]
    fn test_ext_aggregator_input_tokens() {
        // Verify aggregator input tokens
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_points.as_str(),
            "restPoints"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .geom_bind_xform
                .as_str(),
            "geomBindXform"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.influences.as_str(),
            "influences"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .num_influences_per_component
                .as_str(),
            "numInfluencesPerComponent"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .has_constant_influences
                .as_str(),
            "hasConstantInfluences"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .blend_shape_offsets
                .as_str(),
            "blendShapeOffsets"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .blend_shape_offset_ranges
                .as_str(),
            "blendShapeOffsetRanges"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .num_blend_shape_offset_ranges
                .as_str(),
            "numBlendShapeOffsetRanges"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .rest_normals
                .as_str(),
            "restNormals"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .face_vertex_indices
                .as_str(),
            "faceVertexIndices"
        );
        assert_eq!(
            EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
                .has_face_varying_normals
                .as_str(),
            "hasFaceVaryingNormals"
        );
    }

    #[test]
    fn test_ext_computation_input_tokens() {
        // Verify computation input tokens
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights.as_str(),
            "blendShapeWeights"
        );
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms.as_str(),
            "skinningXforms"
        );
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS.skinning_scale_xforms.as_str(),
            "skinningScaleXforms"
        );
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS.skinning_dual_quats.as_str(),
            "skinningDualQuats"
        );
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS
                .skel_local_to_common_space
                .as_str(),
            "skelLocalToWorld"
        );
        assert_eq!(
            EXT_COMPUTATION_INPUT_TOKENS
                .common_space_to_prim_local
                .as_str(),
            "primWorldToLocal"
        );
    }

    #[test]
    fn test_ext_computation_output_tokens() {
        // Verify computation output tokens
        assert_eq!(
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_points.as_str(),
            "skinnedPoints"
        );
        assert_eq!(
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_normals.as_str(),
            "skinnedNormals"
        );

        // Ensure outputs are unique
        assert_ne!(
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_points,
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_normals
        );
    }

    #[test]
    fn test_legacy_tokens() {
        // Verify legacy tokens
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

        // Verify legacy tokens match modern equivalents in value
        assert_eq!(
            EXT_COMPUTATION_LEGACY_INPUT_TOKENS.skel_local_to_world,
            EXT_COMPUTATION_INPUT_TOKENS.skel_local_to_common_space
        );
        assert_eq!(
            EXT_COMPUTATION_LEGACY_INPUT_TOKENS.prim_world_to_local,
            EXT_COMPUTATION_INPUT_TOKENS.common_space_to_prim_local
        );
    }

    #[test]
    fn test_token_consistency() {
        // Test that points and normals computations follow consistent naming
        let points_agg = EXT_COMPUTATION_NAME_TOKENS
            .points_aggregator_computation
            .as_str();
        let normals_agg = EXT_COMPUTATION_NAME_TOKENS
            .normals_aggregator_computation
            .as_str();

        assert!(points_agg.contains("Points"));
        assert!(normals_agg.contains("Normals"));
        assert!(points_agg.contains("Aggregator"));
        assert!(normals_agg.contains("Aggregator"));

        let points_comp = EXT_COMPUTATION_NAME_TOKENS.points_computation.as_str();
        let normals_comp = EXT_COMPUTATION_NAME_TOKENS.normals_computation.as_str();

        assert!(points_comp.contains("Points"));
        assert!(normals_comp.contains("Normals"));
    }
}

#[cfg(test)]
mod adapter_tests {
    use crate::skel::*;

    #[test]
    fn test_module_structure() {
        let _ = &*EXT_COMPUTATION_TYPE_TOKENS;
        let _ = &*PRIM_TYPE_TOKENS;
    }
}
