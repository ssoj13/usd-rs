//! USD Utilities module - utilities for working with USD stages and layers.
//!
//! Port of pxr/usd/usdUtils
//!
//! This module provides utilities for:
//! - Time code ranges and iteration
//! - Layer stack flattening
//! - Asset dependency extraction
//! - Layer stitching and clip stitching
//! - Stage caching
//! - Sparse value writing
//! - Collection authoring
//! - Pipeline conventions
//! - USDZ packaging
//! - Diagnostic delegates

pub mod asset_localization;
pub mod asset_localization_delegate;
pub mod asset_localization_package;
pub mod authoring;
pub mod coalescing_diagnostic_delegate;
pub mod conditional_abort_diagnostic_delegate;
pub mod dependencies;
pub mod fix_broken_pixar_schemas;
pub mod flatten_layer_stack;
pub mod introspection;
pub mod localize_asset;
pub mod pipeline;
pub mod registered_variant_set;
pub mod sparse_value_writer;
pub mod stage_cache;
pub mod stitch;
pub mod stitch_clips;
pub mod time_code_range;
pub mod tokens;
pub mod usdz_package;
pub mod user_processing_func;

// Re-export main types
pub use asset_localization::{LocalizationContext, ReferenceType};
pub use asset_localization_delegate::{
    DependencyType, LocalizationDelegate, ProcessedPathCache, ReadOnlyLocalizationDelegate,
    WritableLocalizationDelegate,
};
pub use asset_localization_package::{
    AssetLocalizationPackage, AssetLocalizationPackageBase, DirectoryRemapper, FileToCopy,
};
pub use authoring::{
    author_collection, compute_collection_includes_and_excludes, copy_layer_metadata,
    create_collections, get_dirty_layers,
};
pub use coalescing_diagnostic_delegate::{
    CoalescingDiagnosticDelegate, CoalescingDiagnosticDelegateItem,
    CoalescingDiagnosticDelegateSharedItem, CoalescingDiagnosticDelegateUnsharedItem,
};
pub use conditional_abort_diagnostic_delegate::{
    ConditionalAbortDiagnosticDelegate, ConditionalAbortDiagnosticDelegateErrorFilters,
};
pub use dependencies::{
    ExtractExternalReferencesParams, ModifyAssetPathFn, compute_all_dependencies,
    extract_external_references, modify_asset_paths,
};
pub use flatten_layer_stack::{
    ResolveAssetPathFn, flatten_layer_stack, flatten_layer_stack_resolve_asset_path,
};
pub use introspection::{UsdStageStatsKeys, compute_usd_stage_stats};
pub use localize_asset::localize_asset;
pub use pipeline::{
    get_alpha_attribute_name_for_color, get_materials_scope_name, get_model_name_from_root_layer,
    get_pref_name, get_prim_at_path_with_forwarding, get_primary_camera_name,
    get_primary_uv_set_name, get_registered_variant_sets, register_variant_set,
    uninstance_prim_at_path,
};
pub use registered_variant_set::{RegisteredVariantSet, SelectionExportPolicy};
pub use sparse_value_writer::{SparseAttrValueWriter, SparseValueWriter};
pub use stage_cache::StageCache;
pub use stitch::{StitchValueFn, StitchValueStatus, stitch_info, stitch_layers};
pub use stitch_clips::{
    generate_clip_manifest_name, generate_clip_topology_name, stitch_clips, stitch_clips_manifest,
    stitch_clips_template, stitch_clips_topology,
};
pub use time_code_range::{TimeCodeRange, TimeCodeRangeIterator};
pub use tokens::{UsdUtilsTokens, tokens};
pub use usdz_package::{create_new_arkit_usdz_package, create_new_usdz_package};
pub use user_processing_func::{DependencyInfo, ProcessingFunc};
