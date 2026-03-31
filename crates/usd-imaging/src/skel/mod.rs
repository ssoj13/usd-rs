//! UsdSkelImaging - Skeletal animation support for USD Imaging.
//!
//! Port of pxr/usdImaging/usdSkelImaging
//!
//! This module provides the infrastructure for imaging skeletal data from USD,
//! including skeletons, skeletal animation, blend shapes, and skinning computations.
//!
//! # Overview
//!
//! UsdSkelImaging bridges the UsdSkel schema (which defines skeletal data in USD)
//! with Hydra (the rendering backend). It provides:
//!
//! - **Skeleton Adapter**: Converts USD Skeleton prims to Hydra representations
//! - **SkelRoot Adapter**: Manages hierarchies of skinned geometry
//! - **Blend Shape Support**: Handles blend shape deformations
//! - **Skinning Computations**: GPU-accelerated skeletal skinning via ext computations
//! - **Scene Index Integration**: Modern scene index-based data flow
//!
//! # Architecture
//!
//! The module uses Hydra's ext computation mechanism to perform skeletal skinning:
//!
//! 1. **Aggregator Computations**: Collect static skinning data (rest poses, bind transforms,
//!    joint influences, blend shape offsets)
//! 2. **Skinning Computations**: Apply animated transforms and blend shape weights to deform
//!    geometry (points and normals)
//!
//! This two-stage approach separates static data collection from dynamic per-frame updates.
//!
//! # Key Concepts
//!
//! **Skeleton**: Defines the joint hierarchy and bind pose of a skeletal rig.
//!
//! **SkelRoot**: A grouping prim that defines the scope for skeletal bindings.
//! All skinned geometry must live under a SkelRoot.
//!
//! **Skinning**: The process of deforming geometry based on joint transforms
//! and influence weights.
//!
//! **Blend Shapes**: Corrective shape targets that blend between different poses,
//! commonly used for facial animation.
//!
//! **Ext Computations**: Hydra's mechanism for GPU computations. UsdSkelImaging
//! uses these to perform skinning on the GPU.
//!
//! # Status
//!
//! This is a foundational implementation providing the token definitions and
//! module structure. Full adapter and scene index implementation requires
//! the completion of the base usdImaging infrastructure.
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use usd_imaging::skel::*;
//!
//! // Access skinning computation tokens
//! let points_comp = &EXT_COMPUTATION_NAME_TOKENS.points_computation;
//! let normals_comp = &EXT_COMPUTATION_NAME_TOKENS.normals_computation;
//!
//! // Access input tokens for ext computations
//! let weights = &EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights;
//! let xforms = &EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms;
//! ```

pub mod glslfx;
pub mod tokens;

pub mod blend_shape_data;
pub mod blend_shape_schema;
pub mod data_source_animation_prim;
pub mod data_source_binding_api;
pub mod data_source_blend_shape_prim;
pub mod data_source_primvar;
pub mod data_source_resolved_ext_computation_prim;
pub mod data_source_resolved_points_based_prim;
pub mod data_source_resolved_skeleton_prim;
pub mod data_source_skeleton_prim;
pub mod data_source_utils;
pub mod ext_computations;
pub mod joint_influences_data;
pub mod resolved_points_based_prim_container;
pub mod resolved_points_based_sources;
pub mod skel_data;
pub mod skel_guide_data;

// Re-export tokens for convenience
pub use tokens::{
    EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS,
    EXT_COMPUTATION_INPUT_TOKENS,
    EXT_COMPUTATION_LEGACY_INPUT_TOKENS,
    EXT_COMPUTATION_NAME_TOKENS,
    EXT_COMPUTATION_OUTPUT_TOKENS,
    // Static token instances
    EXT_COMPUTATION_TYPE_TOKENS,
    ExtAggregatorComputationInputTokens,
    ExtComputationInputTokens,
    ExtComputationLegacyInputTokens,
    ExtComputationNameTokens,
    ExtComputationOutputTokens,
    ExtComputationTypeTokens,
    PRIM_TYPE_TOKENS,
    PrimTypeTokens,
};

pub mod animation_adapter;
pub mod animation_schema;
pub mod binding_api_adapter;
pub mod binding_schema;
pub mod blend_shape_adapter;
pub mod inbetween_shape_schema;
pub mod points_resolving_scene_index;
pub mod resolved_skeleton_schema;
pub mod resolving_scene_index_plugin;
pub mod skel_root_adapter;
pub mod skeleton_adapter;
pub mod skeleton_resolving_scene_index;
pub mod skeleton_schema;
// NOTE: skinning_scene_index.rs removed — wrong architecture.
// C++ uses SkeletonResolvingSceneIndex + PointsResolvingSceneIndex instead
// of a manual registration-based SkinningSceneIndex.
// pub mod skinning_scene_index;
pub mod utils;
pub mod xform_resolver;

pub use animation_adapter::AnimationAdapter;
pub use animation_schema::{AnimationSchema, AnimationSchemaBuilder};
pub use binding_api_adapter::BindingAPIAdapter;
pub use binding_schema::{BindingSchema, BindingSchemaBuilder};
pub use blend_shape_adapter::BlendShapeAdapter;
pub use blend_shape_schema::BlendShapeSchema;
pub use data_source_animation_prim::DataSourceAnimationPrim;
pub use data_source_binding_api::DataSourceBindingAPI;
pub use data_source_blend_shape_prim::DataSourceBlendShapePrim;
pub use data_source_resolved_ext_computation_prim::data_source_resolved_ext_computation_prim;
pub use data_source_resolved_points_based_prim::DataSourceResolvedPointsBasedPrim;
pub use data_source_resolved_skeleton_prim::DataSourceResolvedSkeletonPrim;
pub use data_source_skeleton_prim::DataSourceSkeletonPrim;
pub use inbetween_shape_schema::{InbetweenShapeSchema, InbetweenShapeSchemaBuilder};
pub use points_resolving_scene_index::PointsResolvingSceneIndex;
pub use resolved_skeleton_schema::{ResolvedSkeletonSchema, ResolvedSkeletonSchemaBuilder};
pub use resolving_scene_index_plugin::ResolvingSceneIndexPlugin;
pub use skel_root_adapter::SkelRootAdapter;
pub use skeleton_adapter::SkeletonAdapter;
pub use skeleton_resolving_scene_index::SkeletonResolvingSceneIndex;
pub use skeleton_schema::{SkeletonSchema, SkeletonSchemaBuilder};
pub use utils::{
    compute_bone_joint_indices, compute_bone_points, compute_bone_topology,
    compute_points_for_single_bone,
};
pub use xform_resolver::DataSourceXformResolver;

#[cfg(test)]
mod tests;
