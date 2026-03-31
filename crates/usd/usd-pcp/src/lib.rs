//! Prim Cache Population (PCP) module.
//!
//! PCP provides the composition engine for USD. It is responsible for
//! computing the composed prim index by traversing and evaluating all
//! composition arcs (references, payloads, inherits, specializes, variants).
//!
//! # Key Concepts
//!
//! - **Arc Types**: Different kinds of composition relationships
//!   (reference, payload, inherit, specialize, variant)
//! - **Prim Index**: The result of composition for a single prim,
//!   representing all contributing opinions in strength order
//! - **Layer Stack**: A stack of layers that contribute opinions
//! - **Composition Cache**: Caches composed prim indices for efficiency
//! - **Node**: A site in the composition graph that contributes opinions
//! - **Map Function**: Transforms paths/values across composition arcs
//!
//! # Examples
//!
//! ```
//! use usd_pcp::{ArcType, RangeType};
//!
//! // Check arc type properties
//! let arc = ArcType::Reference;
//! assert!(arc.is_composition_arc());
//! assert!(!arc.is_class_based());
//!
//! // Inherit and specialize are class-based arcs
//! assert!(ArcType::Inherit.is_class_based());
//! assert!(ArcType::Specialize.is_class_based());
//! ```

// Core modules
pub mod arc;
pub mod cache;
pub mod changes;
pub mod compose_site;
pub mod debug_codes;
pub mod dependencies;
pub mod dependency;
pub mod dependent_namespace_edit_utils;
pub mod diagnostic;
pub mod dynamic_file_format;
pub mod dynamic_file_format_dependency_data;
pub mod errors;
pub mod expression_variables;
pub mod expression_variables_dependency_data;
pub mod expression_variables_source;
pub mod indexer;
pub mod instancing;
pub mod iterator;
pub mod layer_relocates_edit_builder;
pub mod layer_stack;
pub mod layer_stack_identifier;
pub mod layer_stack_registry;
pub mod map_expression;
pub mod map_function;
pub mod namespace_edit_type;
pub mod namespace_edits;
pub mod node;
pub mod node_iterator;
pub mod parallel_indexer;
pub mod path_translation;
pub mod prim_index;
pub mod prim_index_graph;
pub mod prim_index_stack_frame;
pub mod property_index;
pub mod site;
pub mod statistics;
pub mod strength_ordering;
pub mod target_index;
pub mod traversal_cache;
pub mod types;
pub mod utils;

// Re-exports - Arc
pub use arc::{Arc, INVALID_NODE_INDEX, NodeIndex};

// Re-exports - Cache
pub use cache::{Cache, CachePtr, PayloadSet};

// Re-exports - Dependency
pub use dependency::{
    Dependency, DependencyFlags, DependencyType, DependencyVector, dependency_flags_to_string,
};

// Re-exports - Errors
pub use errors::{
    ConflictReason, ErrorArcCycle, ErrorArcPermissionDenied, ErrorArcToProhibitedChild,
    ErrorCapacityExceeded, ErrorInconsistentAttributeType, ErrorInconsistentAttributeVariability,
    ErrorInconsistentPropertyType, ErrorInvalidAssetPath, ErrorInvalidAuthoredRelocation,
    ErrorInvalidConflictingRelocation, ErrorInvalidExternalTargetPath,
    ErrorInvalidInstanceTargetPath, ErrorInvalidPrimPath, ErrorInvalidReferenceOffset,
    ErrorInvalidSameTargetRelocations, ErrorInvalidSublayerOffset, ErrorInvalidSublayerOwnership,
    ErrorInvalidSublayerPath, ErrorInvalidTargetPath, ErrorMutedAssetPath,
    ErrorOpinionAtRelocationSource, ErrorPrimPermissionDenied, ErrorPropertyPermissionDenied,
    ErrorSublayerCycle, ErrorTargetPermissionDenied, ErrorType, ErrorUnresolvedPrimPath,
    ErrorVariableExpressionError, PcpError, PcpErrorBasePtr, PcpErrorVector, RelocationSource,
    raise_errors,
};

// Re-exports - Expression Variables
pub use expression_variables::{ExpressionVariables, ExpressionVariablesCachingComposer};
pub use expression_variables_source::ExpressionVariablesSource;

// Re-exports - Layer Stack
pub use layer_stack::{LayerStack, LayerStackPtr, LayerStackRefPtr};
pub use layer_stack_identifier::{LayerStackIdentifier, LayerStackIdentifierHash};

// Re-exports - Map Function / Expression
pub use map_expression::{MapExpression, MapExpressionVariable};
pub use map_function::{MapFunction, PathMap, PathPair, PathPairVector};

// Re-exports - Namespace Edit
pub use namespace_edit_type::NamespaceEditType;

// Re-exports - Node
pub use node::{
    NodeRef, NodeRefHashSet, NodeRefVector, Permission, count_non_variant_path_elements,
};

// Re-exports - Prim Index
pub use prim_index::{
    CompressedSdSite, PayloadState, PrimIndex, PrimIndexInputs, PrimIndexOutputs,
    compute_prim_index,
};

// Re-exports - Prim Index Graph
pub use prim_index_graph::{PrimIndexGraph, PrimIndexGraphPtr, PrimIndexGraphRefPtr};

// Re-exports - Site
pub use site::{Site, SiteHash, SiteSet, SiteVector};

// Re-exports - Types
pub use types::{
    ArcType, INVALID_INDEX, RangeType, SiteTracker, SiteTrackerSegment, VariantFallbackMap,
};

// Re-exports - Compose Site
pub use compose_site::{
    ArcInfo, ArcInfoVector, TokenSet, VariantSelectionMap, compose_site_has_specs,
    compose_site_has_value_clips, compose_site_inherits, compose_site_payloads,
    compose_site_permission, compose_site_prim_sites, compose_site_references,
    compose_site_specializes, compose_site_variant_selection, compose_site_variant_selections,
    compose_site_variant_set_options, compose_site_variant_sets,
};

// Re-exports - Strength Ordering
pub use strength_ordering::{
    compare_node_strength, compare_sibling_node_strength, compare_sibling_payload_node_strength,
};
// These helpers live in utils but are part of the public API.
pub use utils::{find_starting_node_of_class_hierarchy, is_propagated_specializes_node};

// Re-exports - Indexer
pub use indexer::{PrimIndexer, Task, TaskType};

// Re-exports - Path Translation
pub use path_translation::{
    translate_path_from_node_to_root, translate_path_from_node_to_root_using_function,
    translate_path_from_root_to_node, translate_path_from_root_to_node_using_function,
    translate_target_path_from_root_to_node,
};

// Re-exports - Property Index
pub use property_index::{
    PropertyIndex, PropertyInfo, build_prim_property_index, build_property_index,
};

// Re-exports - Target Index
pub use target_index::{
    TargetIndex, TargetSpecType, build_filtered_target_index, build_target_index,
};

// Re-exports - Iterator
pub use iterator::{
    NodeIterator, NodeRange, NodeReverseIterator, PrimIterator, PrimRange, PrimReverseIterator,
    PropertyIterator, PropertyRange, PropertyReverseIterator,
};

// Re-exports - Instancing
pub use instancing::{
    InstanceKey, child_node_instanceable_changed, child_node_is_direct_or_in_direct_arc_subtree,
    child_node_is_instanceable, prim_index_is_instanceable, traverse_instanceable_strong_to_weak,
    traverse_instanceable_weak_to_strong,
};

// Re-exports - Changes
pub use changes::{
    CacheChanges, ChangeSpecsType, Changes, LayerStackChanges, Lifeboat, TargetType,
};

// Re-exports - Dependencies
pub use dependencies::{
    CulledDependency, Dependencies, add_culled_dependencies, node_introduces_dependency,
    node_uses_layer_or_layer_stack, node_uses_layer_or_layer_stack_layer,
};

// Re-exports - Utils
pub use utils::{
    FileFormatArguments, TARGET_ARG, VariableExpressionError, evaluate_variable_expression,
    evaluate_variable_expression_simple,
    find_starting_node_of_class_hierarchy as find_starting_node_of_class_hierarchy_full,
    get_arguments_for_file_format_target, get_arguments_for_file_format_target_into,
    get_arguments_for_file_format_target_stripped,
    get_arguments_for_file_format_target_with_identifier, is_class_based_arc,
    is_propagated_specializes_node as is_propagated_specializes_node_full, is_specialize_arc,
    is_variable_expression, strip_file_format_target, translate_path_from_node_to_root_or_closest,
};

// Re-exports - Diagnostic
pub use diagnostic::{
    IndexingPhaseScope, check_consistency, dump_dot_graph_to_file, dump_node, dump_prim_index,
    format_layer_stack_site, format_site, generate_dot_graph, indexing_msg, indexing_update,
};

// Re-exports - Layer Stack Registry
pub use layer_stack_registry::{
    LayerStackRegistry, LayerStackRegistryPtr, LayerStackRegistryRefPtr, MutedLayers,
};

// Re-exports - Namespace Edits
pub use namespace_edits::{
    CacheSite, EditType, LayerStackSite, NamespaceEdits, compute_delete_edits,
    compute_namespace_edits, compute_rename_edits, compute_reparent_edits, would_create_cycle,
};

// Re-exports - Traversal Cache
pub use traversal_cache::TraversalCache;

// Re-exports - Debug Codes
pub use debug_codes::PcpDebugCode;

// Re-exports - Dynamic File Format
pub use dynamic_file_format::{
    DynamicFileFormatContext, DynamicFileFormatInterface, VtValueVector,
    create_dynamic_file_format_context,
};

// Re-exports - Dynamic File Format Dependency Data
pub use dynamic_file_format_dependency_data::DynamicFileFormatDependencyData;

// Re-exports - Expression Variables Dependency Data
pub use expression_variables_dependency_data::ExpressionVariablesDependencyData;

// Re-exports - Node Iterator
pub use node_iterator::{
    NodeRefPrivateChildrenConstIterator, NodeRefPrivateChildrenConstRange,
    NodeRefPrivateChildrenConstReverseIterator, NodeRefPrivateSubtreeConstIterator,
    NodeRefPrivateSubtreeConstRange, get_children, get_children_range, get_subtree_range,
};

// Re-exports - Prim Index Stack Frame
pub use prim_index_stack_frame::{PrimIndexStackFrame, PrimIndexStackFrameIterator};

// Re-exports - Layer Relocates Edit Builder
pub use layer_relocates_edit_builder::{
    LayerRelocatesEdit, LayerRelocatesEditBuilder, LayerRelocatesEdits, modify_relocates,
};

// Re-exports - Statistics
pub use statistics::{
    ArcTypeCounts, CacheStatistics, PrimIndexStatistics, cache_statistics_string,
    collect_cache_statistics, collect_prim_index_statistics, prim_index_statistics_string,
    print_cache_statistics, print_prim_index_statistics,
};

// Re-exports - Parallel Indexer
pub use parallel_indexer::{ParallelIndexer, ParallelIndexerOutputs};

// Re-exports - Dependent Namespace Edit Utils
pub use dependent_namespace_edit_utils::{
    CompositionFieldEdit, DependentNamespaceEdits, MoveEditDescription, MoveEditDescriptionVector,
    Relocates as DependentRelocates, gather_dependent_namespace_edits,
    gather_layers_to_edit_for_spec_move,
};
