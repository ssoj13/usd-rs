//! Standalone test for usd_imaging::skel module.
//!
//! This file can be compiled independently to verify the module works.

// This would normally be done via `use crate::...` but for standalone testing:
// use super::tokens::*;

#[cfg(test)]
mod standalone_tests {
    // Import tokens from parent module
    use crate::skel::*;
    use usd_tf::Token;

    #[test]
    fn verify_module_compiles() {
        // Basic smoke test - if this compiles, module structure is valid
        assert!(true);
    }

    #[test]
    fn verify_tokens_accessible() {
        // Verify we can access token structs
        let _points = ExtComputationTypeTokens::POINTS;
        let _normals = ExtComputationTypeTokens::NORMALS;
        
        let _skel = PrimTypeTokens::SKELETON;
        let _anim = PrimTypeTokens::SKEL_ANIMATION;
        
        // If we got here, tokens are accessible
        assert!(true);
    }

    #[test]
    fn verify_token_values() {
        // Verify token string values are correct
        assert_eq!(ExtComputationTypeTokens::POINTS.as_str(), "points");
        assert_eq!(ExtComputationTypeTokens::NORMALS.as_str(), "normals");
    }

    #[test]
    fn verify_prim_types() {
        // Verify prim type tokens
        assert_eq!(PrimTypeTokens::SKELETON.as_str(), "skeleton");
        assert_eq!(PrimTypeTokens::SKEL_ANIMATION.as_str(), "skelAnimation");
        assert_eq!(PrimTypeTokens::SKEL_BLEND_SHAPE.as_str(), "skelBlendShape");
    }

    #[test]
    fn verify_computation_names() {
        // Verify ext computation names
        let points_agg = ExtComputationNameTokens::POINTS_AGGREGATOR_COMPUTATION;
        assert_eq!(points_agg.as_str(), "skinningPointsInputAggregatorComputation");
        
        let points_comp = ExtComputationNameTokens::POINTS_COMPUTATION;
        assert_eq!(points_comp.as_str(), "skinningPointsComputation");
    }

    #[test]
    fn verify_input_tokens() {
        // Verify aggregator input tokens
        assert_eq!(
            ExtAggregatorComputationInputNameTokens::REST_POINTS.as_str(),
            "restPoints"
        );
        
        // Verify computation input tokens
        assert_eq!(
            ExtComputationInputNameTokens::BLEND_SHAPE_WEIGHTS.as_str(),
            "blendShapeWeights"
        );
    }

    #[test]
    fn verify_output_tokens() {
        // Verify output tokens
        assert_eq!(
            ExtComputationOutputNameTokens::SKINNED_POINTS.as_str(),
            "skinnedPoints"
        );
        assert_eq!(
            ExtComputationOutputNameTokens::SKINNED_NORMALS.as_str(),
            "skinnedNormals"
        );
    }

    #[test]
    fn verify_token_uniqueness() {
        // Critical: Computation names must be unique for Hydra
        let comp_names = vec![
            ExtComputationNameTokens::POINTS_AGGREGATOR_COMPUTATION,
            ExtComputationNameTokens::POINTS_COMPUTATION,
            ExtComputationNameTokens::NORMALS_AGGREGATOR_COMPUTATION,
            ExtComputationNameTokens::NORMALS_COMPUTATION,
        ];
        
        // Check all pairs are unique
        for i in 0..comp_names.len() {
            for j in (i + 1)..comp_names.len() {
                assert_ne!(
                    comp_names[i], 
                    comp_names[j],
                    "Computation names must be unique: {} vs {}",
                    comp_names[i].as_str(),
                    comp_names[j].as_str()
                );
            }
        }
    }
}

// Main function for standalone compilation test
#[cfg(not(test))]
fn main() {
    println!("usd_imaging::skel module compiles successfully!");
    println!("Run with `cargo test` to execute tests.");
}
