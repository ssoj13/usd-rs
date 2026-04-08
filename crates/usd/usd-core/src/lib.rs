//! USD Core Module - User-facing API for scene description.
//!
//! This module provides the primary API for working with USD scenes:
//!
//! - `UsdStage` - The scene container that presents composed prims
//! - `UsdPrim` - Access to composed prim data
//! - `UsdProperty` - Base class for properties  
//! - `UsdAttribute` - Typed attribute values
//! - `UsdRelationship` - Relationships between prims
//!
//! # Architecture
//!
//! The USD module sits atop SDF (Scene Description Foundation) and PCP
//! (Prim Cache Population). SDF provides the low-level layer data model,
//! PCP handles composition, and USD presents the final composed result.
//!
//! ```text
//!   ┌─────────────────────────────────────────────────────┐
//!   │                    USD (this module)                │
//!   │       UsdStage, UsdPrim, UsdAttribute, etc.         │
//!   ├─────────────────────────────────────────────────────┤
//!   │                        PCP                          │
//!   │            Composition engine, prim indices         │
//!   ├─────────────────────────────────────────────────────┤
//!   │                        SDF                          │
//!   │         Layers, specs, paths, file formats          │
//!   └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Examples
//!
//! ```rust,ignore
//! use usd_core::{UsdStage, InitialLoadSet};
//!
//! // Create a new stage
//! let stage = UsdStage::create_new("HelloWorld.usda", InitialLoadSet::LoadAll)?;
//!
//! // Define a prim
//! let prim = stage.define_prim("/World/Cube", "Cube")?;
//!
//! // Get an attribute
//! let size_attr = prim.get_attribute("size")?;
//! size_attr.set(2.0, TimeCode::default())?;
//!
//! // Save the stage
//! stage.save()?;
//! ```

pub mod api_schema_base;
pub mod attribute;
pub mod attribute_limits;
pub mod attribute_query;
pub mod clip;
pub mod clip_cache;
pub mod clip_set;
pub mod clip_set_definition;
pub mod clips_api;
pub mod collection_api;
pub mod collection_membership_query;
pub mod collection_predicate_library;
pub mod color_space_api;
pub mod color_space_definition_api;
pub mod common;
pub mod compose_time_sample_series;
pub mod edit_context;
pub mod edit_target;
pub mod errors;
pub mod flatten_utils;
pub mod inherits;
pub mod instance_cache;
pub mod instance_key;
pub mod interpolation;
pub mod interpolators;
pub mod load_rules;
pub mod model_api;
pub mod namespace_editor;
pub mod notice;
pub mod object;
pub mod payloads;
pub mod population_mask;
pub mod prim;
pub mod prim_composition_query;
pub mod prim_data;
pub mod prim_definition;
pub mod prim_flags;
pub mod prim_range;
pub mod prim_type_info;
pub mod prim_type_info_cache;
pub mod property;
pub mod references;
pub mod relationship;
pub mod resolve_info;
pub mod resolve_target;
pub mod resolver;
pub mod schema_base;
pub mod schema_registry;
pub mod schema_traits;
pub mod specializes;
pub mod stage;
pub mod stage_cache;
pub mod stage_cache_context;
pub mod time_code;
pub mod time_sample_resolvers;
pub mod tokens;
pub mod typed;
pub mod value_utils;
pub mod variant_sets;

// Re-exports
pub use attribute::Attribute;
pub use attribute_query::AttributeQuery;
pub use clip::{get_clip_related_fields, is_clip_related_field};
pub use collection_predicate_library::get_collection_predicate_library;
pub use color_space_definition_api::ColorSpaceDefinitionAPI;
pub use common::{
    InitialLoadSet, ListPosition, LoadPolicy, SchemaKind, SchemaVersion, VersionPolicy,
};
pub use edit_context::{EditContext, EditTargetGuard};
pub use edit_target::EditTarget;
pub use errors::{ExpiredPrimAccessError, UsdError};
pub use inherits::Inherits;
pub use interpolation::{
    InterpolationType, get_stage_interpolation_type, set_stage_interpolation_type,
};
pub use load_rules::{Rule as LoadRule, StageLoadRules};
pub use model_api::ModelAPI;
pub use notice::{
    ChangeEntry, LayerMutingChanged, NamespaceEditsInfo, ObjectsChanged, PathRange, PrimResyncInfo,
    PrimResyncType, StageContentsChanged, StageEditTargetChanged, StageNotice,
};
pub use object::Object;
pub use payloads::Payloads;
pub use population_mask::StagePopulationMask;
pub use prim::Prim;
pub use prim_composition_query::Filter as CompositionQueryFilter;
pub use prim_composition_query::{
    ArcIntroducedFilter, ArcTypeFilter, DependencyTypeFilter, Filter, HasSpecsFilter,
    PrimCompositionQuery, PrimCompositionQueryArc,
};
pub use prim_data::{PrimData, PrimDataHandle, PrimTypeInfo};
pub use prim_definition::{
    Attribute as PrimAttribute, PrimDefinition, Property as PrimDefinitionProperty,
    Relationship as PrimRelationship,
};
pub use prim_flags::{PrimFlags, PrimFlagsPredicate};
pub use prim_range::{PrimRange, PrimRangeIterator};
pub use prim_type_info::{PrimTypeId, PrimTypeInfo as PrimTypeInfoFull};
pub use prim_type_info_cache::PrimTypeInfoCache;
pub use property::Property;
pub use references::References;
pub use relationship::Relationship;
pub use resolve_info::{ResolveInfo, ResolveInfoSource};
pub use resolve_target::ResolveTarget;
pub use schema_base::SchemaBase;
pub use schema_registry::{SchemaInfo, SchemaRegistry, TokenToTokenVectorMap};
pub use schema_traits::{SchemaAttrInfo, UsdAPISchema, UsdSchemaBase, UsdTyped};
pub use specializes::Specializes;
pub use stage::Stage;
pub use stage_cache::{StageCache, StageCacheId};
pub use stage_cache_context::{
    StageCacheContext, StageCacheContextBlockType, use_but_do_not_populate_cache,
};
pub use time_code::{TimeCode, tokens as time_code_tokens};
pub use tokens::usd_tokens;
pub use typed::Typed;
pub use value_utils::{
    DefaultValueResult, clear_value_if_blocked, copy_time_samples_in_interval, insert_list_item,
    merge_time_samples,
};
pub use variant_sets::{VariantSet, VariantSets};
