//! USD Skel module - skeletal animation and skinning support.
//!
//! Port of pxr/usd/usdSkel
//!
//! This module provides types for:
//! - Skeletons (joint hierarchies)
//! - Skeletal animations (joint transforms over time)
//! - Skinning (binding geometry to skeletons)
//! - Blend shapes (morph targets)

// Core modules
pub mod anim_query;
pub mod animation;
pub mod tokens;
pub mod topology;

// Schema modules
pub mod blend_shape;
pub mod inbetween_shape;
pub mod root;
pub mod skeleton;

// Additional modules
pub mod anim_mapper;
pub mod binding;
pub mod binding_api;
pub mod skinning_query;
pub mod utils;

// Complex API modules - now enabled
pub mod bake_skinning;
pub mod blend_shape_query;
pub mod cache;
pub mod skel_definition;
pub mod skeleton_query;

// Re-export main types
pub use anim_query::AnimQuery;
pub use animation::SkelAnimation;
pub use blend_shape::BlendShape;
pub use inbetween_shape::InbetweenShape;
pub use root::SkelRoot;
pub use skeleton::Skeleton;
pub use tokens::{UsdSkelTokens, tokens};
pub use topology::Topology;

pub use anim_mapper::AnimMapper;
pub use binding::Binding;
pub use binding_api::BindingAPI;
pub use skinning_query::SkinningQuery;

// Re-export complex API types
pub use bake_skinning::{
    BakeSkinningParams, DeformationFlags, bake_skinning, bake_skinning_for_prims,
    bake_skinning_for_root,
};
pub use blend_shape_query::BlendShapeQuery;
pub use cache::Cache;
pub use skel_definition::SkelDefinition;
pub use skeleton_query::SkeletonQuery;
