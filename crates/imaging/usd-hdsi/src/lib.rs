
//! HDSI - Hydra Scene Index Utilities
//!
//! Port of pxr/imaging/hdsi
//!
//! This module provides scene index filters and utilities for the Hydra
//! rendering framework. Scene indices form a filtering chain that transforms
//! scene data for consumption by render delegates.
//!
//! # Scene Index Filters
//!
//! ## Core Filters
//! - [`HdsiCoordSysPrimSceneIndex`] - Creates coordinate system prims
//! - [`HdsiImplicitSurfaceSceneIndex`] - Converts implicit surfaces to mesh
//! - [`HdsiMaterialBindingResolvingSceneIndex`] - Resolves material bindings
//! - [`HdsiSceneGlobalsSceneIndex`] - Manages scene globals
//!
//! ## Pruning Filters
//! - [`HdsiPrimTypePruningSceneIndex`] - Prunes prims by type
//! - [`HdsiPrimTypeAndPathPruningSceneIndex`] - Prunes by type and path
//! - [`HdsiPrefixPathPruningSceneIndex`] - Prunes by path prefix
//! - [`HdsiSceneMaterialPruningSceneIndex`] - Prunes unused materials
//! - [`HdsiUnboundMaterialPruningSceneIndex`] - Prunes unbound materials
//! - [`HdsiRenderPassPruneSceneIndex`] - Prunes render pass prims
//!
//! ## Material Filters
//! - [`HdsiMaterialOverrideResolvingSceneIndex`] - Resolves material overrides
//! - [`HdsiMaterialPrimvarTransferSceneIndex`] - Transfers primvars from materials
//! - [`HdsiMaterialRenderContextFilteringSceneIndex`] - Filters by render context
//!
//! ## Light Filters
//! - [`HdsiLightLinkingSceneIndex`] - Resolves light linking
//! - [`HdsiDomeLightCameraVisibilitySceneIndex`] - Manages dome light visibility
//!
//! ## Computation Filters
//! - [`HdsiExtComputationDependencySceneIndex`] - Manages computation dependencies
//! - [`HdsiExtComputationPrimvarPruningSceneIndex`] - Prunes computed primvars
//!
//! ## Geometry Filters
//! - [`HdsiNurbsApproximatingSceneIndex`] - Converts NURBS to mesh
//! - [`HdsiPinnedCurveExpandingSceneIndex`] - Expands pinned curves
//! - [`HdsiTetMeshConversionSceneIndex`] - Converts tetrahedral meshes
//!
//! ## Other Filters
//! - [`HdsiLegacyDisplayStyleOverrideSceneIndex`] - Applies display style overrides
//! - [`HdsiNodeIdentifierResolvingSceneIndex`] - Resolves node identifiers
//! - [`HdsiPrimTypeNoticeBatchingSceneIndex`] - Batches notices by prim type
//! - [`HdsiRenderSettingsFilteringSceneIndex`] - Filters render settings
//! - [`HdsiSwitchingSceneIndex`] - Switches between scene indices
//! - [`HdsiVelocityMotionResolvingSceneIndex`] - Resolves velocity motion blur
//!
//! ## Debugging
//! - [`HdsiDebuggingSceneIndex`] - Checks for inconsistencies in input scene (no transformation)
//!
//! ## Observers
//! - [`HdsiPrimManagingSceneIndexObserver`] - Manages prim lifecycle
//!
//! # Utilities
//! - [`compute_scene_index_diff`] - Computes differences between scene indices
//! - [`utils`] - Utility functions

// Scene index filters
pub mod coord_sys_prim_scene_index;
pub mod implicit_surface_scene_index;
pub mod implicit_to_mesh;
pub mod light_linking_scene_index;
pub mod material_binding_resolving_scene_index;
pub mod prim_type_pruning_scene_index;
pub mod render_settings_filtering_scene_index;
pub mod scene_globals_scene_index;

pub mod dome_light_camera_visibility_scene_index;
pub mod ext_computation_dependency_scene_index;
pub mod ext_computation_primvar_pruning_scene_index;
pub mod legacy_display_style_override_scene_index;
pub mod material_override_resolving_scene_index;
pub mod material_primvar_transfer_scene_index;
pub mod material_render_context_filtering_scene_index;
pub mod node_identifier_resolving_scene_index;
pub mod nurbs_approximating_scene_index;
pub mod pinned_curve_expanding_scene_index;
pub mod prefix_path_pruning_scene_index;
pub mod prim_managing_scene_index_observer;
pub mod prim_type_and_path_pruning_scene_index;
pub mod prim_type_notice_batching_scene_index;
pub mod render_pass_prune_scene_index;
pub mod scene_material_pruning_scene_index;
pub mod switching_scene_index;
pub mod tet_mesh_conversion_scene_index;
pub mod unbound_material_pruning_scene_index;
pub mod velocity_motion_resolving_scene_index;

// Utilities
pub mod compute_scene_index_diff;
pub mod debugging_scene_index;
pub mod debugging_scene_index_plugin;
pub mod utils;
pub mod version;

// Tokens
pub mod tokens;

// Re-exports
pub use coord_sys_prim_scene_index::HdsiCoordSysPrimSceneIndex;
pub use implicit_surface_scene_index::HdsiImplicitSurfaceSceneIndex;
pub use light_linking_scene_index::HdsiLightLinkingSceneIndex;
pub use material_binding_resolving_scene_index::HdsiMaterialBindingResolvingSceneIndex;
pub use prim_type_pruning_scene_index::HdsiPrimTypePruningSceneIndex;
pub use render_settings_filtering_scene_index::HdsiRenderSettingsFilteringSceneIndex;
pub use scene_globals_scene_index::HdsiSceneGlobalsSceneIndex;

pub use dome_light_camera_visibility_scene_index::HdsiDomeLightCameraVisibilitySceneIndex;
pub use ext_computation_dependency_scene_index::HdsiExtComputationDependencySceneIndex;
pub use ext_computation_primvar_pruning_scene_index::HdsiExtComputationPrimvarPruningSceneIndex;
pub use legacy_display_style_override_scene_index::{
    HdsiLegacyDisplayStyleOverrideSceneIndex, OptionalInt,
};
pub use material_override_resolving_scene_index::HdsiMaterialOverrideResolvingSceneIndex;
pub use material_primvar_transfer_scene_index::HdsiMaterialPrimvarTransferSceneIndex;
pub use material_render_context_filtering_scene_index::HdsiMaterialRenderContextFilteringSceneIndex;
pub use node_identifier_resolving_scene_index::HdsiNodeIdentifierResolvingSceneIndex;
pub use nurbs_approximating_scene_index::HdsiNurbsApproximatingSceneIndex;
pub use pinned_curve_expanding_scene_index::HdsiPinnedCurveExpandingSceneIndex;
pub use prefix_path_pruning_scene_index::HdsiPrefixPathPruningSceneIndex;
pub use prim_managing_scene_index_observer::{
    HdsiPrimManagingSceneIndexObserver, PrimBase, PrimBaseHandle, PrimFactoryBase,
    PrimFactoryBaseHandle,
};
pub use prim_type_and_path_pruning_scene_index::{
    HdsiPrimTypeAndPathPruningSceneIndex, PathPredicate,
};
pub use prim_type_notice_batching_scene_index::{
    HdPrimTypePriorityFunctorDataSource, HdsiPrimTypeNoticeBatchingSceneIndex,
    PrimTypePriorityFunctor,
};
pub use render_pass_prune_scene_index::HdsiRenderPassPruneSceneIndex;
pub use scene_material_pruning_scene_index::HdsiSceneMaterialPruningSceneIndex;
pub use switching_scene_index::HdsiSwitchingSceneIndex;
pub use tet_mesh_conversion_scene_index::HdsiTetMeshConversionSceneIndex;
pub use unbound_material_pruning_scene_index::HdsiUnboundMaterialPruningSceneIndex;
pub use velocity_motion_resolving_scene_index::HdsiVelocityMotionResolvingSceneIndex;

pub use debugging_scene_index::HdsiDebuggingSceneIndex;
pub use debugging_scene_index_plugin::HdsiDebuggingSceneIndexPlugin;

pub use compute_scene_index_diff::{
    ComputeSceneIndexDiffFn, SceneIndexDiff, compute_scene_index_diff,
    compute_scene_index_diff_at_path, compute_scene_index_diff_delta,
    compute_scene_index_diff_delta_fn, compute_scene_index_diff_root,
};
pub use tokens::*;
pub use utils::{
    HdCollectionExpressionEvaluator, compile_collection, is_pruned, remove_pruned_children,
};
pub use version::HDSI_API_VERSION;

#[cfg(test)]
mod tests;
