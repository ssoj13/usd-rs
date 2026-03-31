//! Scene Description Foundation (SDF) module.
//!
//! SDF provides the low-level data model for USD. It defines the foundational
//! types used to describe scene data, including:
//!
//! - `Path` - Addresses locations in the scene graph hierarchy
//! - `AssetPath` - References to external assets
//! - `TimeCode` - Time values for animation
//!
//! # Path Syntax
//!
//! SDF paths use a syntax similar to file paths:
//!
//! - `/` - Absolute root
//! - `.` - Relative root (current prim)
//! - `/Foo` - Absolute prim path
//! - `/Foo/Bar` - Nested prim path
//! - `/Foo.prop` - Property path
//! - `/Foo.rel[/Target]` - Relationship target
//! - `/Foo{variant=selection}` - Variant selection
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{Path, TimeCode, AssetPath};
//!
//! // Create paths
//! let prim_path = Path::from_string("/World/Cube").unwrap();
//! let prop_path = prim_path.append_property("visibility").unwrap();
//!
//! // Create time codes
//! let time = TimeCode::new(1.0);
//!
//! // Create asset paths
//! let asset = AssetPath::new("model.usd");
//! ```

pub mod abc_data;
pub mod abc_file_format;
pub mod abc_reader;
pub mod abc_util;
pub mod abc_writer;
pub mod abstract_data;
pub mod accessor_helpers;
pub mod allowed;
pub mod asset_path;
pub mod asset_path_resolver;
pub mod attribute_spec;
pub mod boolean_expression;
pub mod change_block;
pub mod change_list;
pub mod change_manager;
pub mod change_type;
pub mod children_policies;
pub mod children_proxy;
pub mod children_view;
pub mod cleanup_enabler;
pub mod cleanup_tracker;
pub mod copy_utils;
pub mod crate_info;
pub mod data;
pub mod file_format;
pub mod file_version;
pub mod identity;
pub mod integer_coding;
pub mod layer;
pub mod layer_hints;
pub mod layer_offset;
pub mod layer_state_delegate;
pub mod layer_tree;
pub mod layer_utils;
pub mod list_editor;
pub mod list_editor_proxy;
pub mod list_op;
pub mod list_proxy;
pub mod map_edit_proxy;
pub mod map_editor;
pub mod namespace_edit;
pub mod notice;
pub mod path;
pub mod path_expression;
pub mod path_expression_eval;
pub mod path_node;
pub mod path_pattern;
pub mod payload;
pub mod predicate_expression;
pub mod predicate_library;
pub mod predicate_program;
pub mod prim_spec;
pub mod property_spec;
pub mod proxy_policies;
pub mod proxy_types;
pub mod reference;
pub mod relationship_spec;
pub mod schema;
pub mod site;
pub mod site_utils;
pub mod spec;
pub mod text_parser;
pub mod time_code;
pub mod tokens;
pub mod types;
pub mod usd_file_format;
pub mod usda_reader;
pub mod usdc_reader;
pub mod usdz_file_format;
pub mod value_type_name;
pub mod value_type_registry;
pub mod variable_expression;
pub mod variant_set_spec;
pub mod variant_spec;
pub mod zip_file;

// Re-exports
pub use abstract_data::{
    AbstractData, DataVisitor, SimpleData, SpecVisitor, Value, create_simple_data,
};
pub use allowed::Allowed;
// AssetPath and AssetPathParams are now defined in usd-vt; re-export for backward compat.
pub use usd_vt::{AssetPath, AssetPathParams};
// Local module still provides Layer-level utility functions (anchor_asset_paths etc.).
pub use asset_path::{anchor_asset_paths, resolve_asset_paths, swap as swap_asset_paths};
pub use attribute_spec::AttributeSpec;
pub use boolean_expression::{BinaryOperator, BooleanExpression, UnaryOperator};
pub use change_block::ChangeBlock;
pub use change_list::{
    ChangeList, Entry as ChangeEntry, EntryFlags, SubLayerChangeType as ChangeListSubLayerType,
};
pub use change_manager::ChangeManager;
pub use change_type::{ChangeFlags, SubLayerChangeType};
pub use children_view::{
    AsKey, ChildPredicate, ChildrenView, NameChildrenView, PrimChildrenView, PropertyChildrenView,
    TrivialPredicate, prim_children, property_children,
};
pub use cleanup_enabler::CleanupEnabler;
pub use cleanup_tracker::CleanupTracker;
pub use copy_utils::{
    CopyFieldResult, CopySpecsValueEdit, ShouldCopyChildrenFn, ShouldCopyFieldFn, copy_spec,
    copy_spec_with_callbacks, remap_path, should_copy_children, should_copy_value,
};
pub use crate_info::{CrateInfo, CrateSection, CrateSummaryStats};
pub use data::{Data, create_data};
pub use file_format::{
    FileFormat, FileFormatArguments, FileFormatError, FileFormatRegistry, find_format_by_extension,
    find_format_by_id, get_all_file_extensions, get_all_formats, get_dynamic_file_format,
    get_file_extension, get_format_registry, is_dynamic_file_format, register_file_format,
};
pub use file_version::FileVersion;
pub use identity::{Identity, IdentityHandle, IdentityRegistry};
pub use layer::Layer;
pub use layer_hints::LayerHints;
pub use layer_offset::{LayerOffset, LayerOffsetVector};
pub use layer_tree::{LayerTree, LayerTreeBuilder, LayerTreeHandle, LayerTreeHandleVector};
pub use layer_utils::{
    compute_asset_path_relative_to_layer, create_identifier_with_args, get_extension,
    get_layer_directory, identifiers_equal, make_relative_path,
    resolve_asset_path_relative_to_layer, split_identifier,
};
pub use list_op::{
    Int64ListOp, IntListOp, ListOp, ListOpType, PathListOp, PayloadListOp, ReferenceListOp,
    StringListOp, TokenListOp, UInt64ListOp, UIntListOp, UnregisteredValueListOp,
    apply_list_ordering,
};
pub use namespace_edit::{
    AT_END, BatchNamespaceEdit, NamespaceEdit, NamespaceEditDetail, NamespaceEditDetailVector,
    NamespaceEditResult, NamespaceEditVector, SAME, combine_error, combine_result,
    combine_unbatched,
};
pub use notice::{
    BaseLayersDidChange, LayerChangeListPair, LayerChangeListVec, LayerDidReloadContent,
    LayerDidReplaceContent, LayerDidSaveLayerToFile, LayerDirtinessChanged,
    LayerIdentifierDidChange, LayerInfoDidChange, LayerMutenessChanged, LayersDidChange,
    LayersDidChangeSentPerLayer, SdfNotice,
};
pub use path::{Path, PathSet, PathVector};
pub use path_expression::{
    ExpressionReference, PathExpression, PathExpressionOp, PathPattern, PatternComponent,
};
pub use path_pattern::{PathPatternComponent, SdfPathPattern};
pub use payload::{Payload, PayloadVector, find_payload_by_identity};
pub use predicate_expression::{FnArg, FnCall, FnCallKind, PredicateExpression, PredicateOp};
pub use predicate_library::{
    Constancy, FromValue, PredicateFunction, PredicateFunctionResult as PredicateLibFunctionResult,
    PredicateLibrary, PredicateParam, PredicateParamNamesAndDefaults, PredicateProgram,
    link_predicate_expression, try_bind_args,
};
pub use prim_spec::{PrimSpec, apply_ordering};
pub use property_spec::PropertySpec;
pub use reference::{Reference, ReferenceVector, find_reference_by_identity};
pub use relationship_spec::RelationshipSpec;
pub use schema::{FieldDefinition, Schema, SchemaBase, SpecDefinition, Validator};
pub use site::{LayerHandle, Site, SiteSet, SiteVector};
pub use site_utils::{
    get_field, get_field_or, get_prim_at_path, get_property_at_path, get_site_identifier,
    has_field, has_spec, is_valid_site, list_fields,
};
pub use spec::{Spec, VtDictionary, VtValue};
// TimeCode is now defined in usd-vt; re-export for backward compat.
pub use tokens::{
    SdfMetadataDisplayGroupTokens, SdfPathTokens, SdfTokens, metadata_display_group_tokens,
    path_chars, path_tokens, sdf_tokens,
};
pub use types::{
    AngularUnit,
    AnimationBlock,
    AuthoringError,
    DimensionlessUnit,
    HumanReadableValue,
    LengthUnit,
    OpaqueValue,
    Permission,
    Relocate,
    RelocatesMap,
    SpecType,
    Specifier,
    TupleDimensions,
    UnitKind,
    ValueBlock,
    ValueRole,
    Variability,
    VariantSelectionMap,
    VariantsMap,
    // Unit conversion free functions (SdfDefaultUnit, SdfConvertUnit, etc.)
    convert_unit,
    default_unit,
    default_unit_for_type,
    get_name_for_unit,
    // Value type validation free functions (SdfValueHasValidType, etc.)
    get_role_name_for_value_type_name,
    get_type_for_value_type_name,
    get_unit_from_name,
    get_value_type_name_for_value,
    unit_category,
    value_has_valid_type,
};
pub use usd_file_format::{UsdFileFormat, register_usd_format};
pub use usd_vt::TimeCode;
pub use usda_reader::{UsdaData, UsdaFileFormat, register_usda_format};
pub use usdc_reader::{CrateData, CrateHeader, UsdcFileFormat, register_usdc_format};
pub use usdz_file_format::{UsdzFileFormat, register_usdz_format};
pub use value_type_name::{
    TupleDimensions as ValueTupleDimensions, ValueTypeName, ValueTypeNameHash,
};
pub use value_type_registry::{ValueTypeBuilder, ValueTypeRegistry};
pub use variable_expression::{
    VariableExpression, VariableExpressionBuilder, VariableExpressionResult,
};
pub use variant_set_spec::{VariantSetSpec, VariantSetsProxy, VariantSpec};
pub use variant_spec::{VariantSpec as SdfVariantSpec, create_variant_in_layer};
pub use zip_file::{FileInfo as ZipFileInfo, ZipError, ZipFile, ZipFileWriter};

// Extend vt::Value with From<ListOp<Path>> (defined here to avoid circular dependency)
// From implementations for ListOp types
// Note: These use direct Value::new() which should work if ListOp<T> implements
// the required traits (Clone + Send + Sync + Debug + PartialEq + Hash)
impl From<PathListOp> for usd_vt::Value {
    #[inline]
    fn from(value: PathListOp) -> Self {
        Self::new(value)
    }
}

impl From<ReferenceListOp> for usd_vt::Value {
    #[inline]
    fn from(value: ReferenceListOp) -> Self {
        Self::new(value)
    }
}

impl From<PayloadListOp> for usd_vt::Value {
    #[inline]
    fn from(value: PayloadListOp) -> Self {
        Self::new(value)
    }
}

// Extend vt::Value with From<TokenListOp>
impl From<TokenListOp> for usd_vt::Value {
    #[inline]
    fn from(value: TokenListOp) -> Self {
        Self::new(value)
    }
}

// From<PathExpression> lives here (usd-sdf) to avoid the circular dep:
// usd-vt -> usd-sdf -> usd-vt.
impl From<PathExpression> for usd_vt::Value {
    #[inline]
    fn from(value: PathExpression) -> Self {
        Self::new(value)
    }
}
use std::sync::Once;

static INIT_FILE_FORMATS: Once = Once::new();

/// Initialize all built-in file formats.
///
/// This registers the following file formats:
/// - `.usda` - USD ASCII text format
/// - `.usdc` - USD binary crate format
/// - `.usd` - USD auto-detect format (reads both, writes as configured)
/// - `.usdz` - USD zip archive format
/// - `.abc` - Alembic format (read-only)
///
/// This function is idempotent and can be called multiple times safely.
/// It is automatically called by CLI tools but library users may need
/// to call it explicitly before opening layers.
///
/// # Example
///
/// ```
/// usd_sdf::init();
/// let layer = usd_sdf::Layer::find_or_open("model.usda");
/// ```
pub fn init() {
    INIT_FILE_FORMATS.call_once(|| {
        register_usda_format();
        register_usdc_format();
        register_usd_format();
        register_usdz_format();
        abc_file_format::register_abc_format();
    });
}

// ============================================================================
// Convenience free functions (C++ SdfCreatePrimInLayer, etc.)
// ============================================================================

/// Create a prim at the given path, and any necessary parent prims, in the
/// given layer.
///
/// If a prim already exists at the given path it will be returned unmodified.
/// New specs are created with `SdfSpecifierOver` and an empty type.
/// `prim_path` must be a valid prim path.
pub fn create_prim_in_layer(layer: &LayerHandle, prim_path: &Path) -> Option<PrimSpec> {
    let layer_arc = layer.upgrade()?;

    // If prim already exists, return it
    if let Some(existing) = layer.get_prim_at_path(prim_path) {
        return Some(existing);
    }

    // Ensure all ancestor prims exist
    let parent_path = prim_path.get_parent_path();
    if !parent_path.is_absolute_root_path() && !parent_path.is_empty() {
        // Recursively create parent
        create_prim_in_layer(layer, &parent_path)?;
    }

    // Create this prim as SdfSpecifierOver with empty type
    layer_arc.create_prim_spec(prim_path, Specifier::Over, "")
}

/// Create a prim at the given path, and any necessary parent prims, in the
/// given layer.
///
/// If a prim already exists at the given path, do nothing and return true.
/// New specs are created with `SdfSpecifierOver` and an empty type.
/// Returns false and issues an error if creation fails.
pub fn just_create_prim_in_layer(layer: &LayerHandle, prim_path: &Path) -> bool {
    create_prim_in_layer(layer, prim_path).is_some()
}

/// Create an attribute spec on a prim spec at the given path, and any
/// necessary parent prim specs, in the given layer.
///
/// If an attribute spec already exists at the given path, update its
/// type_name, variability, and custom fields and return it.
/// `attr_path` must be a valid prim property path.
pub fn create_prim_attribute_in_layer(
    layer: &LayerHandle,
    attr_path: &Path,
    type_name: &str,
    variability: Variability,
    is_custom: bool,
) -> Option<AttributeSpec> {
    if !attr_path.is_prim_property_path() {
        return None;
    }

    let layer_arc = layer.upgrade()?;

    // Ensure parent prim exists
    let prim_path = attr_path.get_prim_path();
    just_create_prim_in_layer(layer, &prim_path);

    // Create or update the attribute spec
    layer_arc.create_spec(attr_path, SpecType::Attribute);

    // Set fields: typeName, variability, custom
    let type_token = usd_tf::Token::new("typeName");
    let var_token = usd_tf::Token::new("variability");
    let custom_token = usd_tf::Token::new("custom");

    layer.set_field(
        attr_path,
        &type_token,
        abstract_data::Value::new(type_name.to_string()),
    );
    layer.set_field(
        attr_path,
        &var_token,
        abstract_data::Value::new(variability),
    );
    layer.set_field(
        attr_path,
        &custom_token,
        abstract_data::Value::new(is_custom),
    );

    Some(AttributeSpec::from_layer_and_path(
        layer.clone(),
        attr_path.clone(),
    ))
}

/// Create an attribute spec on a prim spec at the given path, and any
/// necessary parent prim specs, in the given layer.
///
/// Returns true on success, false on failure.
pub fn just_create_prim_attribute_in_layer(
    layer: &LayerHandle,
    attr_path: &Path,
    type_name: &str,
    variability: Variability,
    is_custom: bool,
) -> bool {
    create_prim_attribute_in_layer(layer, attr_path, type_name, variability, is_custom).is_some()
}

/// Create a relationship spec on a prim spec at the given path, and any
/// necessary parent prim specs, in the given layer.
///
/// If a relationship spec already exists at the given path, update its
/// variability and custom fields and return it.
/// `rel_path` must be a valid prim property path.
pub fn create_relationship_in_layer(
    layer: &LayerHandle,
    rel_path: &Path,
    variability: Variability,
    is_custom: bool,
) -> Option<RelationshipSpec> {
    if !rel_path.is_prim_property_path() {
        return None;
    }

    let layer_arc = layer.upgrade()?;

    // Ensure parent prim exists
    let prim_path = rel_path.get_prim_path();
    just_create_prim_in_layer(layer, &prim_path);

    // Create or update the relationship spec
    layer_arc.create_spec(rel_path, SpecType::Relationship);

    // Set fields: variability, custom
    let var_token = usd_tf::Token::new("variability");
    let custom_token = usd_tf::Token::new("custom");

    layer.set_field(rel_path, &var_token, abstract_data::Value::new(variability));
    layer.set_field(
        rel_path,
        &custom_token,
        abstract_data::Value::new(is_custom),
    );

    Some(RelationshipSpec::from_spec(Spec::new(
        layer.clone(),
        rel_path.clone(),
    )))
}

/// Create a relationship spec on a prim spec at the given path, and any
/// necessary parent prim specs, in the given layer.
///
/// Returns true on success, false on failure.
pub fn just_create_relationship_in_layer(
    layer: &LayerHandle,
    rel_path: &Path,
    variability: Variability,
    is_custom: bool,
) -> bool {
    create_relationship_in_layer(layer, rel_path, variability, is_custom).is_some()
}

pub use accessor_helpers::{
    AccessorBuilder, AllowAllReads, AllowAllWrites, FieldAccessor, GuardedAccessor, ReadPredicate,
    TypedFieldAccessor, WritePredicate,
};
pub use children_policies::{
    AttributeChildPolicy, ChildPolicy, PathChildPolicy, PrimChildPolicy, PropertyChildPolicy,
    RelationshipChildPolicy, TokenChildPolicy, VariantChildPolicy, VariantSetChildPolicy,
};
pub use children_proxy::{
    AttributeChildrenProxy as ProxyAttributeChildren, ChildrenProxy, ChildrenProxyError,
    ChildrenProxyResult, PrimChildrenProxy as ProxyPrimChildren,
    PropertyChildrenProxy as ProxyPropertyChildren,
    RelationshipChildrenProxy as ProxyRelationshipChildren,
    VariantChildrenProxy as ProxyVariantChildren,
    VariantSetChildrenProxy as ProxyVariantSetChildren,
};
pub use list_editor_proxy::{ApplyCallback, ListEditorProxy, ModifyCallback};
pub use list_proxy::{
    INVALID_INDEX as LIST_PROXY_INVALID_INDEX, ListProxy, ListProxyError, ListProxyResult,
};
pub use map_edit_proxy::{
    DictionaryProxy, MapEditProxy, MapEditProxyError, MapEditProxyResult, TokenDictionaryProxy,
};
pub use proxy_policies::{
    ChildPredicate as ProxyChildPredicate, KeyPolicy, NameKeyPolicy, NameTokenKeyPolicy,
    PathKeyPolicy, PathTypePolicy, PayloadTypePolicy, ReferenceTypePolicy, StringTypePolicy,
    SubLayerTypePolicy, TokenTypePolicy, TrivialPredicate as ProxyTrivialPredicate, TypePolicy,
    VtValuePolicy,
};
pub use proxy_types::{
    ConnectionsProxy, NameChildrenOrderProxy, PathListEditorProxy, PathListProxy,
    PayloadListEditorProxy, PayloadListProxy, PayloadsProxy, PropertyOrderProxy,
    ReferenceListEditorProxy, ReferenceListProxy, ReferencesProxy, RelocatesMapProxy,
    StringListEditorProxy, StringListProxy, StringMapProxy, SubLayerListEditorProxy,
    SubLayerListProxy, SubLayersProxy, TargetsProxy, TokenListEditorProxy, TokenListProxy,
    VariantSelectionsProxy,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all exports are accessible
        let _path = Path::empty();
        let _asset = AssetPath::empty();
        let _time = TimeCode::new(0.0);
        let _tokens = path_tokens();
    }

    #[test]
    fn test_path_roundtrip() {
        let original = Path::from_string("/World/Cube").unwrap();
        let string = original.get_string();
        let parsed = Path::from_string(string).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_path_hierarchy() {
        let root = Path::absolute_root();
        let world = root.append_child("World").unwrap();
        let cube = world.append_child("Cube").unwrap();
        let visibility = cube.append_property("visibility").unwrap();

        assert!(visibility.is_property_path());
        assert_eq!(visibility.get_prim_path(), cube);
        assert_eq!(cube.get_parent_path(), world);
        assert_eq!(world.get_parent_path(), root);
    }

    #[test]
    fn test_time_code_arithmetic() {
        let t1 = TimeCode::new(10.0);
        let t2 = TimeCode::new(5.0);

        assert_eq!((t1 + t2).value(), 15.0);
        assert_eq!((t1 - t2).value(), 5.0);
        assert_eq!((t1 * t2).value(), 50.0);
        assert_eq!((t1 / t2).value(), 2.0);
    }

    #[test]
    fn test_asset_path_parts() {
        let path = AssetPath::from_params(
            AssetPathParams::new()
                .authored("model_{VAR}.usd")
                .evaluated("model_a.usd")
                .resolved("/root/model_a.usd"),
        );

        assert_eq!(path.get_authored_path(), "model_{VAR}.usd");
        assert_eq!(path.get_evaluated_path(), "model_a.usd");
        assert_eq!(path.get_resolved_path(), "/root/model_a.usd");
        assert_eq!(path.get_asset_path(), "model_a.usd");
    }
}
