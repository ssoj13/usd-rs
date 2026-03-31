//! UsdFlattenUtils - utilities for flattening layer stacks.
//!
//! Port of pxr/usd/usd/flattenUtils.h/cpp
//!
//! Utilities for flattening layer stacks into a single layer.

use crate::clips_api::ClipsAPIInfoKeys;
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use usd_gf::Vec2d;
use usd_pcp::LayerStack;
use usd_pcp::compose_site::compose_site_child_names;
use usd_sdf::list_op::ListOp;
use usd_sdf::payload::Payload;
use usd_sdf::reference::Reference;
use usd_sdf::{
    AssetPath, Layer, LayerOffset, Path, SpecType, Specifier, ValueBlock, VariantSetSpec,
    variable_expression::VariableExpression,
};
use usd_tf::Token;
use usd_vt::{Array, Dictionary, Value, Vec2dArray};

// ============================================================================
// ResolveAssetPathContext
// ============================================================================

/// Context object containing information used when resolving asset paths
/// during layer stack flattening.
///
/// Matches C++ `UsdFlattenResolveAssetPathContext`.
#[derive(Debug, Clone)]
pub struct ResolveAssetPathContext {
    /// Layer where the asset path is authored.
    pub source_layer: Option<Arc<Layer>>,
    /// Authored asset path.
    pub asset_path: String,
    /// Expression variables from the layer stack.
    pub expression_variables: Dictionary,
}

// ============================================================================
// Helper Type Aliases
// ============================================================================

type ResolveAssetPathFn = Box<dyn Fn(&Arc<Layer>, &str) -> String>;
type ResolveAssetPathAdvancedFn = Box<dyn Fn(&ResolveAssetPathContext) -> String>;

// ============================================================================
// Internal Helper Functions
// ============================================================================

/// Reduces two list ops by applying operations.
///
/// Matches C++ `_Reduce(const SdfListOp<T>& lhs, const SdfListOp<T>& rhs)`.
fn reduce_list_op<T: Clone + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync + 'static>(
    lhs: &ListOp<T>,
    rhs: &ListOp<T>,
) -> Value {
    // We assume the caller has already applied _FixListOp()
    // Try to compose the list ops
    let mut result = lhs.clone();
    result.compose_stronger(rhs);
    Value::new(result)
}

/// Reduces two dictionaries by composing keys recursively.
///
/// Matches C++ `_Reduce(const VtDictionary& lhs, const VtDictionary& rhs)`.
fn reduce_dictionary(lhs: &Dictionary, rhs: &Dictionary) -> Value {
    // Dictionaries compose keys recursively.
    let mut result = lhs.clone();
    for (key, value) in rhs.iter() {
        if let Some(lhs_value) = result.get(key) {
            // Recursively compose if both are dictionaries
            if let Some(lhs_dict) = lhs_value.get::<Dictionary>() {
                if let Some(rhs_dict) = value.get::<Dictionary>() {
                    let composed = reduce_dictionary(lhs_dict, rhs_dict);
                    result.insert(key.clone(), composed);
                    continue;
                }
            }
        }
        // Otherwise, stronger (rhs) wins
        result.insert(key.clone(), value.clone());
    }
    Value::new(result)
}

/// Reduces two specifiers.
///
/// Matches C++ `_Reduce(const SdfSpecifier& lhs, const SdfSpecifier& rhs)`.
fn reduce_specifier(lhs: Specifier, rhs: Specifier) -> Value {
    // SdfSpecifierOver is the equivalent of "no opinion"
    // However, in the case of composing strictly within a layer stack,
    // they can be considered as strongest wins.
    if lhs == Specifier::Over {
        Value::new(rhs)
    } else {
        Value::new(lhs)
    }
}

/// Reduces two values, handling type-specific composition rules.
///
/// Matches C++ `_Reduce(const VtValue& lhs, const VtValue& rhs, const TfToken& field)`.
fn reduce_value(lhs: &Value, rhs: &Value, field: &Token) -> Value {
    // Handle easy generic cases first.
    if lhs.is_empty() {
        return rhs.clone();
    }
    if rhs.is_empty() {
        return lhs.clone();
    }

    // Check for ValueBlock
    if lhs.get::<ValueBlock>().is_some() || rhs.get::<ValueBlock>().is_some() {
        // If the stronger value is a block, return it;
        // if the weaker value is a block, return the stronger value.
        return lhs.clone();
    }

    // Check for AnimationBlock
    use usd_sdf::types::AnimationBlock;
    if lhs.get::<AnimationBlock>().is_some() || rhs.get::<AnimationBlock>().is_some() {
        // If either value is an AnimationBlock, return the stronger value
        // AnimationBlock blocks animation from weaker layers
        return lhs.clone();
    }

    // Check if types match
    if lhs.type_name() != rhs.type_name() {
        // If the types do not match, there is no reduction rule for
        // combining them, so just use the stronger value.
        return lhs.clone();
    }

    // Dispatch to type-specific reduce / compose rules.
    // Try list ops first
    if let Some(lhs_list) = lhs.get::<ListOp<i32>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<i32>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<u32>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<u32>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<i64>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<i64>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<u64>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<u64>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<Token>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<Token>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<String>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<String>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<Path>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<Path>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<Reference>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<Reference>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }
    if let Some(lhs_list) = lhs.get::<ListOp<Payload>>() {
        if let Some(rhs_list) = rhs.get::<ListOp<Payload>>() {
            return reduce_list_op(lhs_list, rhs_list);
        }
    }

    // Try dictionaries
    if let Some(lhs_dict) = lhs.get::<Dictionary>() {
        if let Some(rhs_dict) = rhs.get::<Dictionary>() {
            return reduce_dictionary(lhs_dict, rhs_dict);
        }
    }

    // Try specifier
    if let Some(lhs_spec) = lhs.get::<Specifier>() {
        if let Some(rhs_spec) = rhs.get::<Specifier>() {
            return reduce_specifier(*lhs_spec, *rhs_spec);
        }
    }

    // TypeName is a special case: empty token represents "no opinion".
    // (That is not true of token-valued fields in general.)
    let type_name_token = Token::new("typeName");
    if field == &type_name_token {
        if let Some(lhs_token) = lhs.get::<Token>() {
            if lhs_token.as_str().is_empty() {
                return rhs.clone();
            }
            return lhs.clone();
        }
    }

    // Generic base case: take stronger opinion.
    lhs.clone()
}

/// "Fixes" a list op to only use composable features.
///
/// Matches C++ `_FixListOp(SdfListOp<T> op)`.
fn fix_list_op<T: Clone + Eq + std::hash::Hash + std::fmt::Debug>(mut op: ListOp<T>) -> ListOp<T> {
    if op.is_explicit() {
        return op;
    }

    // Convert added items to appended items
    let mut items = op.get_appended_items().to_vec();
    for item in op.get_added_items() {
        if !items.contains(item) {
            items.push(item.clone());
        }
    }

    // Set appended items and clear added/ordered items
    op.set_appended_items(items).ok();
    op.set_added_items(Vec::new());
    op.set_ordered_items(Vec::new());

    op
}

/// Fixes list op values in a VtValue.
///
/// Matches C++ `_FixListOpValue(VtValue* v)`.
fn fix_list_op_value(val: &mut Value) {
    // Try to fix various list op types
    if let Some(list_op) = val.get::<ListOp<i32>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<u32>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<i64>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<u64>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<Token>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<String>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<Path>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<Reference>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
        return;
    }
    if let Some(list_op) = val.get::<ListOp<Payload>>().cloned() {
        *val = Value::new(fix_list_op(list_op));
    }
}

/// Applies layer offset to clip info.
///
/// Matches C++ `_ApplyLayerOffsetToClipInfo`.
fn apply_layer_offset_to_clip_info(
    offset: &LayerOffset,
    info_key: &Token,
    clip_info: &mut Dictionary,
) {
    if let Some(v) = clip_info.get_mut(info_key.as_str()) {
        // Try to get Vec2dArray
        if let Some(array) = v.get::<Vec2dArray>().cloned() {
            // Apply offset to first component of each Vec2d
            let mut vec = array.into_vec();
            for entry in &mut vec {
                // entry[0] = offset * entry[0]
                let new_time = offset.apply(entry.x);
                *entry = Vec2d::new(new_time, entry.y);
            }
            *v = Value::from_no_hash(Vec2dArray::from(vec));
        }
    }
}

/// Applies layer offset to reference or payload.
///
/// Matches C++ `_ApplyLayerOffsetToRefOrPayload`.
fn apply_layer_offset_to_ref_or_payload(
    offset: &LayerOffset,
    ref_or_payload: &Reference,
) -> Option<Reference> {
    let mut result = ref_or_payload.clone();
    let new_offset = *offset * *ref_or_payload.layer_offset();
    result.set_layer_offset(new_offset);
    Some(result)
}

/// Applies layer offset to payload.
///
/// Matches C++ `_ApplyLayerOffsetToRefOrPayload` for Payload.
fn apply_layer_offset_to_payload(offset: &LayerOffset, payload: &Payload) -> Option<Payload> {
    let mut result = payload.clone();
    let new_offset = *offset * *payload.layer_offset();
    result.set_layer_offset(new_offset);
    Some(result)
}

/// Applies layer offsets (time remapping) to time-keyed metadata.
///
/// Matches C++ `_ApplyLayerOffset(const SdfLayerOffset& offset, const TfToken& field, VtValue* val)`.
fn apply_layer_offset(offset: &LayerOffset, field: &Token, val: &mut Value) {
    if offset.is_identity() {
        return;
    }

    // Handle clips field
    let clips_token = Token::new("clips");
    if field == &clips_token {
        if let Some(clips_dict) = val.get::<Dictionary>().cloned() {
            // Apply offset to clip info for each clip set
            let mut new_clips_dict = clips_dict.clone();
            let clip_set_names: Vec<String> = new_clips_dict.keys().cloned().collect();
            for clip_set_name in clip_set_names {
                if let Some(clip_info_val) = new_clips_dict.get(&clip_set_name) {
                    if let Some(mut clip_info) = clip_info_val.get::<Dictionary>().cloned() {
                        // Apply offset to active times
                        apply_layer_offset_to_clip_info(
                            offset,
                            &ClipsAPIInfoKeys::active(),
                            &mut clip_info,
                        );
                        // Apply offset to times
                        apply_layer_offset_to_clip_info(
                            offset,
                            &ClipsAPIInfoKeys::times(),
                            &mut clip_info,
                        );
                        new_clips_dict.insert_value(clip_set_name.clone(), Value::new(clip_info));
                    }
                }
            }
            *val = Value::new(new_clips_dict);
        }
    }

    // Handle References field
    let references_token = Token::new("references");
    if field == &references_token {
        if let Some(mut refs) = val.get::<ListOp<Reference>>().cloned() {
            // Apply offset to each reference in all lists
            let modified_explicit = refs
                .get_explicit_items()
                .iter()
                .filter_map(|r| apply_layer_offset_to_ref_or_payload(offset, r))
                .collect();
            let modified_prepended = refs
                .get_prepended_items()
                .iter()
                .filter_map(|r| apply_layer_offset_to_ref_or_payload(offset, r))
                .collect();
            let modified_appended = refs
                .get_appended_items()
                .iter()
                .filter_map(|r| apply_layer_offset_to_ref_or_payload(offset, r))
                .collect();

            if refs.is_explicit() {
                refs.set_explicit_items(modified_explicit).ok();
            } else {
                refs.set_prepended_items(modified_prepended).ok();
                refs.set_appended_items(modified_appended).ok();
            }
            *val = Value::new(refs);
        }
    }

    // Handle Payload field
    let payload_token = Token::new("payload");
    if field == &payload_token {
        if let Some(mut pls) = val.get::<ListOp<Payload>>().cloned() {
            // Apply offset to each payload in all lists
            let modified_explicit = pls
                .get_explicit_items()
                .iter()
                .filter_map(|p| apply_layer_offset_to_payload(offset, p))
                .collect();
            let modified_prepended = pls
                .get_prepended_items()
                .iter()
                .filter_map(|p| apply_layer_offset_to_payload(offset, p))
                .collect();
            let modified_appended = pls
                .get_appended_items()
                .iter()
                .filter_map(|p| apply_layer_offset_to_payload(offset, p))
                .collect();

            if pls.is_explicit() {
                pls.set_explicit_items(modified_explicit).ok();
            } else {
                pls.set_prepended_items(modified_prepended).ok();
                pls.set_appended_items(modified_appended).ok();
            }
            *val = Value::new(pls);
        }
    }

    // For other fields, apply layer offset to time samples if present
    // This would call Usd_ApplyLayerOffsetToValue
    // For now, skip this as it requires more complex time sample handling
}

/// Fixes asset paths in a value recursively.
///
/// Helper function that calls fix_asset_paths with empty field token.
fn fix_asset_paths_in_value(
    source_layer: &Arc<Layer>,
    val: &mut Value,
    resolve_asset_path_fn: &ResolveAssetPathFn,
) {
    fix_asset_paths(source_layer, &Token::new(""), resolve_asset_path_fn, val);
}

/// Fixes asset paths in a value.
///
/// Matches C++ `_FixAssetPaths`.
fn fix_asset_paths(
    source_layer: &Arc<Layer>,
    _field: &Token,
    resolve_asset_path_fn: &ResolveAssetPathFn,
    val: &mut Value,
) {
    // Handle SdfAssetPath
    if let Some(asset_path) = val.get::<usd_sdf::AssetPath>().cloned() {
        let resolved = resolve_asset_path_fn(source_layer, asset_path.get_asset_path());
        *val = Value::new(usd_sdf::AssetPath::new(resolved));
        return;
    }

    // Handle Array<SdfAssetPath>
    if let Some(asset_paths) = val.get::<Array<AssetPath>>().cloned() {
        // Resolve each asset path in the array
        let resolved_paths: Vec<AssetPath> = asset_paths
            .iter()
            .map(|ap| {
                let resolved = resolve_asset_path_fn(source_layer, ap.get_asset_path());
                AssetPath::new(&resolved)
            })
            .collect();
        *val = Value::new(Array::from(resolved_paths));
        return;
    }

    // Handle SdfTimeSampleMap with asset paths
    // TimeSampleMap is BTreeMap<OrderedFloat<f64>, Value>
    // We need to check if values contain AssetPath and resolve them
    if let Some(time_samples) = val.get::<BTreeMap<OrderedFloat<f64>, Value>>().cloned() {
        let mut resolved_samples = BTreeMap::new();
        for (time, sample_val) in time_samples {
            let mut resolved_val = sample_val.clone();
            // Recursively resolve asset paths in the value
            fix_asset_paths_in_value(source_layer, &mut resolved_val, resolve_asset_path_fn);
            resolved_samples.insert(time, resolved_val);
        }
        *val = Value::new(resolved_samples);
        return;
    }

    // Handle SdfReference
    if let Some(mut ref_val) = val.get::<Reference>().cloned() {
        let resolved = resolve_asset_path_fn(source_layer, ref_val.asset_path());
        ref_val.set_asset_path(resolved);
        // layer_offset is preserved from original reference
        *val = Value::new(ref_val);
        return;
    }

    // Handle SdfReferenceListOp
    if let Some(refs) = val.get::<ListOp<Reference>>().cloned() {
        // Apply fix to each reference using apply_operations with callback
        let mut ref_vec = Vec::new();
        refs.apply_operations(
            &mut ref_vec,
            Some(|_op_type, ref_item: &Reference| {
                let mut fixed_ref = ref_item.clone();
                let resolved = resolve_asset_path_fn(source_layer, fixed_ref.asset_path());
                fixed_ref.set_asset_path(resolved);
                Some(fixed_ref)
            }),
        );
        // Reconstruct ListOp from fixed references
        let mut fixed_refs = ListOp::<Reference>::new();
        fixed_refs.set_appended_items(ref_vec).ok();
        *val = Value::new(fixed_refs);
        return;
    }

    // Handle SdfPayload
    if let Some(mut pl) = val.get::<Payload>().cloned() {
        let resolved = resolve_asset_path_fn(source_layer, pl.asset_path());
        pl.set_asset_path(resolved);
        // layer_offset is preserved from original payload
        *val = Value::new(pl);
        return;
    }

    // Handle SdfPayloadListOp
    if let Some(pls) = val.get::<ListOp<Payload>>().cloned() {
        // Apply fix to each payload using apply_operations with callback
        let mut pl_vec = Vec::new();
        pls.apply_operations(
            &mut pl_vec,
            Some(|_op_type, pl_item: &Payload| {
                let mut fixed_pl = pl_item.clone();
                let resolved = resolve_asset_path_fn(source_layer, fixed_pl.asset_path());
                fixed_pl.set_asset_path(resolved);
                Some(fixed_pl)
            }),
        );
        // Reconstruct ListOp from fixed payloads
        let mut fixed_pls = ListOp::<Payload>::new();
        fixed_pls.set_appended_items(pl_vec).ok();
        *val = Value::new(fixed_pls);
        return;
    }

    // Handle clips field
    let clips_token = Token::new("clips");
    if _field == &clips_token {
        if let Some(clips_dict) = val.get::<Dictionary>().cloned() {
            // Fix asset paths in clip info - iterate through clip sets
            // Matches C++ _FixAssetPaths for clips field (flattenUtils.cpp:431-463)
            use crate::clips_api::ClipsAPIInfoKeys;
            use std::collections::HashMap;

            let mut fixed_clips_map = HashMap::<String, Value>::new();

            // Iterate through each clip set entry
            for (clip_set_name, clip_info_val) in clips_dict.iter() {
                // Each entry should be a dictionary (clip info)
                if let Some(clip_info_map) = clip_info_val.as_dictionary() {
                    let mut fixed_clip_info_map = clip_info_map.clone();

                    // Fix assetPaths array
                    let asset_paths_key = ClipsAPIInfoKeys::asset_paths().get_text().to_string();
                    if let Some(mut asset_paths_val) =
                        fixed_clip_info_map.get(&asset_paths_key).cloned()
                    {
                        if asset_paths_val.is::<Array<AssetPath>>() {
                            fix_asset_paths(
                                source_layer,
                                &ClipsAPIInfoKeys::asset_paths(),
                                resolve_asset_path_fn,
                                &mut asset_paths_val,
                            );
                            fixed_clip_info_map.insert(asset_paths_key, asset_paths_val);
                        }
                    }

                    // Fix manifestAssetPath
                    let manifest_key = ClipsAPIInfoKeys::manifest_asset_path()
                        .get_text()
                        .to_string();
                    if let Some(mut manifest_path_val) =
                        fixed_clip_info_map.get(&manifest_key).cloned()
                    {
                        if manifest_path_val.is::<AssetPath>() {
                            fix_asset_paths(
                                source_layer,
                                &ClipsAPIInfoKeys::manifest_asset_path(),
                                resolve_asset_path_fn,
                                &mut manifest_path_val,
                            );
                            fixed_clip_info_map.insert(manifest_key, manifest_path_val);
                        }
                    }

                    // Update the clip info in the dictionary
                    fixed_clips_map.insert(
                        clip_set_name.clone(),
                        Value::from_dictionary(fixed_clip_info_map),
                    );
                } else {
                    // Not a dictionary, keep as-is
                    fixed_clips_map.insert(clip_set_name.clone(), clip_info_val.clone());
                }
            }

            *val = Value::from_dictionary(fixed_clips_map);
        }
    }
}

/// Reduces a field across all layers in the stack.
///
/// Matches C++ `_ReduceField`.
fn reduce_field(
    layer_stack: &Arc<LayerStack>,
    target_spec_path: &Path,
    spec_type: SpecType,
    field: &Token,
    resolve_asset_path_fn: &ResolveAssetPathFn,
) -> Value {
    let layers = layer_stack.get_layers();
    let mut val = Value::empty();

    for (i, layer) in layers.iter().enumerate() {
        if !layer.has_spec(target_spec_path) {
            continue;
        }

        // Ignore mismatched specs
        if layer.get_spec_type(target_spec_path) != spec_type {
            continue;
        }

        let Some(mut layer_val) = layer.get_field(target_spec_path, field) else {
            continue;
        };

        // Apply layer offsets
        if let Some(offset) = layer_stack.get_layer_offset_at(i) {
            apply_layer_offset(&offset, field, &mut layer_val);
        }

        // Fix asset paths
        fix_asset_paths(layer, field, resolve_asset_path_fn, &mut layer_val);

        // Fix any list ops
        fix_list_op_value(&mut layer_val);

        // Reduce with accumulated value
        val = reduce_value(&val, &layer_val, field);
    }

    val
}

/// Fields to skip during flattening.
///
/// Matches C++ `_fieldsToSkip`.
fn fields_to_skip() -> BTreeSet<Token> {
    let mut skip = BTreeSet::new();

    // SdfChildrenKeys fields are maintained internally by Sdf
    skip.insert(Token::new("primChildren"));
    skip.insert(Token::new("properties"));
    skip.insert(Token::new("variantSetChildren"));
    skip.insert(Token::new("variantChildren"));

    // We need to go through the SdfListEditorProxy API to
    // properly create attribute connections and rel targets,
    // so don't process the fields.
    skip.insert(Token::new("targetPaths"));
    skip.insert(Token::new("connectionPaths"));

    // We flatten out sublayers, so discard them.
    skip.insert(Token::new("subLayers"));
    skip.insert(Token::new("subLayerOffsets"));

    // TimeSamples may be masked by Defaults, so handle them separately.
    skip.insert(Token::new("timeSamples"));

    // Splines may also be masked by Defaults, so handle them separately.
    skip.insert(Token::new("spline"));

    skip
}

/// Flattens fields for a spec.
///
/// Matches C++ `_FlattenFields`.
fn flatten_fields(
    layer_stack: &Arc<LayerStack>,
    target_layer: &Arc<Layer>,
    target_spec_path: &Path,
    spec_type: SpecType,
    resolve_asset_path_fn: &ResolveAssetPathFn,
) {
    let schema = target_layer.get_schema();
    let fields = schema
        .base()
        .get_spec_def(spec_type)
        .map(|def| def.get_fields())
        .unwrap_or_default();
    let skip_fields = fields_to_skip();

    for field in fields {
        if skip_fields.contains(&field) {
            continue;
        }

        let val = reduce_field(
            layer_stack,
            target_spec_path,
            spec_type,
            &field,
            resolve_asset_path_fn,
        );
        if !val.is_empty() {
            // Value in abstract_data is just an alias for vt::Value
            target_layer.set_field(target_spec_path, &field, val);
        }
    }

    // Handle TimeSamples and Spline for attributes separately
    if spec_type == SpecType::Attribute {
        let time_samples_token = Token::new("timeSamples");
        let spline_token = Token::new("spline");
        let default_token = Token::new("default");

        let layers = layer_stack.get_layers();
        for layer in layers.iter() {
            let mut processed = false;

            if layer.has_field(target_spec_path, &time_samples_token) {
                let val = reduce_field(
                    layer_stack,
                    target_spec_path,
                    spec_type,
                    &time_samples_token,
                    resolve_asset_path_fn,
                );
                if !val.is_empty() {
                    target_layer.set_field(target_spec_path, &time_samples_token, val);
                }
                processed = true;
            }

            if layer.has_field(target_spec_path, &spline_token) {
                let val = reduce_field(
                    layer_stack,
                    target_spec_path,
                    spec_type,
                    &spline_token,
                    resolve_asset_path_fn,
                );
                if !val.is_empty() {
                    target_layer.set_field(target_spec_path, &spline_token, val);
                }
                processed = true;
            }

            if processed {
                break;
            }

            // Check if this layer has defaults that mask time samples/spline
            if layer.has_field(target_spec_path, &default_token) {
                break;
            }
        }
    }
}

/// Gets the spec type at a site across layers.
///
/// Matches C++ `_GetSiteSpecType`.
fn get_site_spec_type(layers: &[Arc<Layer>], path: &Path) -> SpecType {
    for layer in layers {
        if layer.has_spec(path) {
            return layer.get_spec_type(path);
        }
    }
    SpecType::Unknown
}

/// Flattens target paths (connections/targets) for a spec.
///
/// Matches C++ `_FlattenTargetPaths`.
fn flatten_target_paths(
    layer_stack: &Arc<LayerStack>,
    target_layer: &Arc<Layer>,
    spec_path: &Path,
    field: &Token,
    resolve_asset_path_fn: &ResolveAssetPathFn,
) {
    let spec_type = target_layer.get_spec_type(spec_path);
    let val = reduce_field(
        layer_stack,
        spec_path,
        spec_type,
        field,
        resolve_asset_path_fn,
    );

    if let Some(list_op) = val.get::<ListOp<Path>>() {
        // Apply the list op to the target paths
        // For attributes, use connection paths
        // For relationships, use target paths
        if spec_type == SpecType::Attribute {
            if let Some(mut attr_spec) = target_layer.get_attribute_at_path(spec_path) {
                // Create a new PathListOp from the reduced list op
                let mut target_list = ListOp::<Path>::new();
                if list_op.is_explicit() {
                    target_list
                        .set_explicit_items(list_op.get_explicit_items().to_vec())
                        .ok();
                } else {
                    target_list
                        .set_prepended_items(list_op.get_prepended_items().to_vec())
                        .ok();
                    target_list
                        .set_appended_items(list_op.get_appended_items().to_vec())
                        .ok();
                    target_list
                        .set_deleted_items(list_op.get_deleted_items().to_vec())
                        .ok();
                }
                attr_spec.set_connection_paths_list(target_list);
            }
        } else if spec_type == SpecType::Relationship {
            if let Some(mut rel_spec) = target_layer.get_relationship_at_path(spec_path) {
                // Create a new PathListOp from the reduced list op
                let mut target_list = ListOp::<Path>::new();
                if list_op.is_explicit() {
                    target_list
                        .set_explicit_items(list_op.get_explicit_items().to_vec())
                        .ok();
                } else {
                    target_list
                        .set_prepended_items(list_op.get_prepended_items().to_vec())
                        .ok();
                    target_list
                        .set_appended_items(list_op.get_appended_items().to_vec())
                        .ok();
                    target_list
                        .set_deleted_items(list_op.get_deleted_items().to_vec())
                        .ok();
                }
                rel_spec.set_target_path_list(target_list);
            }
        }
    }
}

/// Flattens a prim spec recursively.
///
/// Matches C++ `_FlattenSpec(const PcpLayerStackRefPtr& layerStack, const SdfPrimSpecHandle& prim, ...)`.
fn flatten_spec_prim(
    layer_stack: &Arc<LayerStack>,
    target_layer: &Arc<Layer>,
    prim_path: &Path,
    resolve_asset_path_fn: &ResolveAssetPathFn,
) {
    let layers = layer_stack.get_layers();

    // Flatten fields for this prim
    flatten_fields(
        layer_stack,
        target_layer,
        prim_path,
        SpecType::Prim,
        resolve_asset_path_fn,
    );

    // Handle pseudo root - no children
    if prim_path == &Path::absolute_root() {
        return;
    }

    // Child prims
    let prim_children_token = Token::new("primChildren");
    let prim_order_token = Token::new("primOrder");
    let (name_order, _name_set) = compose_site_child_names(
        &layers,
        prim_path,
        &prim_children_token,
        Some(&prim_order_token),
    );

    for child_name in name_order {
        if let Some(child_path) = prim_path.append_child(child_name.as_str()) {
            // Create child prim spec with placeholder specifier
            if target_layer
                .create_prim_spec(&child_path, Specifier::Def, "")
                .is_some()
            {
                flatten_fields(
                    layer_stack,
                    target_layer,
                    &child_path,
                    SpecType::Prim,
                    resolve_asset_path_fn,
                );
                flatten_spec_prim(
                    layer_stack,
                    target_layer,
                    &child_path,
                    resolve_asset_path_fn,
                );
            }
        }
    }

    // Variant sets
    let variant_set_children_token = Token::new("variantSetChildren");
    let (variant_set_names, _) =
        compose_site_child_names(&layers, prim_path, &variant_set_children_token, None);

    for vset_name in variant_set_names {
        // Get or create prim spec for variant set creation
        if let Some(prim_spec) = target_layer.get_prim_at_path(prim_path) {
            // Create variant set spec
            if let Ok(variant_set) = VariantSetSpec::new(&prim_spec, vset_name.as_str()) {
                // Variant set spec is created, now flatten its fields
                let vset_path = variant_set.path();
                flatten_fields(
                    layer_stack,
                    target_layer,
                    &vset_path,
                    SpecType::VariantSet,
                    resolve_asset_path_fn,
                );
            }
        }
    }

    // Properties
    let property_children_token = Token::new("properties");
    let (property_names, _) =
        compose_site_child_names(&layers, prim_path, &property_children_token, None);

    for prop_name in property_names {
        if let Some(prop_path) = prim_path.append_property(prop_name.as_str()) {
            let spec_type = get_site_spec_type(&layers, &prop_path);

            if spec_type == SpecType::Attribute {
                // Create attribute spec with placeholder type
                if target_layer.create_spec(&prop_path, SpecType::Attribute) {
                    // Set placeholder type
                    let type_name_token = Token::new("typeName");
                    let int_type = Token::new("int");
                    let type_value = Value::new(int_type);
                    target_layer.set_field(&prop_path, &type_name_token, type_value);

                    flatten_fields(
                        layer_stack,
                        target_layer,
                        &prop_path,
                        SpecType::Attribute,
                        resolve_asset_path_fn,
                    );

                    // Flatten connection paths
                    let connection_paths_token = Token::new("connectionPaths");
                    flatten_target_paths(
                        layer_stack,
                        target_layer,
                        &prop_path,
                        &connection_paths_token,
                        resolve_asset_path_fn,
                    );
                }
            } else if spec_type == SpecType::Relationship {
                // Create relationship spec
                if target_layer.create_spec(&prop_path, SpecType::Relationship) {
                    flatten_fields(
                        layer_stack,
                        target_layer,
                        &prop_path,
                        SpecType::Relationship,
                        resolve_asset_path_fn,
                    );

                    // Flatten target paths
                    let target_paths_token = Token::new("targetPaths");
                    flatten_target_paths(
                        layer_stack,
                        target_layer,
                        &prop_path,
                        &target_paths_token,
                        resolve_asset_path_fn,
                    );
                }
            }
        }
    }
}

/// Evaluates an asset path expression.
///
/// Matches C++ `_EvaluateAssetPathExpression`.
fn evaluate_asset_path_expression(expression: &str, expression_vars: &Dictionary) -> String {
    // Check if this is a variable expression
    if !VariableExpression::is_expression(expression) {
        return expression.to_string();
    }

    // Parse and evaluate the expression
    let expr = VariableExpression::new(expression);
    let result = expr.evaluate(expression_vars);

    // Check for errors
    if !result.errors.is_empty() {
        // Log warning (in C++ this uses TF_WARN)
        // For now, just return the original expression
        return expression.to_string();
    }

    // Extract the evaluated string value
    if let Some(value) = result.value.as_ref() {
        if let Some(s) = value.get::<String>() {
            return s.clone();
        }
    }

    // Fallback to original expression
    expression.to_string()
}

// ============================================================================
// Public API Functions
// ============================================================================

/// Flatten layerStack into a single layer with the given optional tag.
///
/// Matches C++ `UsdFlattenLayerStack(const PcpLayerStackRefPtr &layerStack, const std::string& tag)`.
///
/// A composed UsdStage created from this flattened layer will be the same
/// as a composed UsdStage whose root layer stack is the original layer stack.
pub fn flatten_layer_stack(layer_stack: &Arc<LayerStack>, tag: Option<&str>) -> Option<Arc<Layer>> {
    flatten_layer_stack_advanced(
        layer_stack,
        Box::new(|ctx: &ResolveAssetPathContext| {
            flatten_layer_stack_resolve_asset_path_advanced(ctx)
        }),
        tag,
    )
}

/// Flatten the layerStack into a single layer using resolveAssetPathFn to resolve asset paths.
///
/// Matches C++ `UsdFlattenLayerStack(const PcpLayerStackRefPtr &layerStack, const UsdFlattenResolveAssetPathFn& resolveAssetPathFn, const std::string& tag)`.
pub fn flatten_layer_stack_with_resolver(
    layer_stack: &Arc<LayerStack>,
    resolve_asset_path_fn: ResolveAssetPathFn,
    tag: Option<&str>,
) -> Option<Arc<Layer>> {
    // Wrap the resolve function to evaluate expressions
    let advanced_fn: ResolveAssetPathAdvancedFn = Box::new(move |ctx: &ResolveAssetPathContext| {
        let asset_path =
            if evaluate_asset_path_expression(&ctx.asset_path, &ctx.expression_variables)
                != ctx.asset_path
            {
                evaluate_asset_path_expression(&ctx.asset_path, &ctx.expression_variables)
            } else {
                ctx.asset_path.clone()
            };

        if let Some(ref source_layer) = ctx.source_layer {
            resolve_asset_path_fn(source_layer, &asset_path)
        } else {
            asset_path
        }
    });

    flatten_layer_stack_advanced(layer_stack, advanced_fn, tag)
}

/// Flatten the layerStack using advanced resolveAssetPathFn.
///
/// Matches C++ `UsdFlattenLayerStack(const PcpLayerStackRefPtr &layerStack, const UsdFlattenResolveAssetPathAdvancedFn& resolveAssetPathFn, const std::string& tag)`.
pub fn flatten_layer_stack_advanced(
    layer_stack: &Arc<LayerStack>,
    resolve_asset_path_fn: ResolveAssetPathAdvancedFn,
    tag: Option<&str>,
) -> Option<Arc<Layer>> {
    use usd_pcp::ExpressionVariables;

    // Compute expression variables for this layer stack
    let identifier = layer_stack.identifier();
    let layer_stack_expr_vars = ExpressionVariables::compute(identifier, identifier, None);

    // Wrap the resolve function to pass along expression variables
    let resolve_fn_wrapper: ResolveAssetPathFn =
        Box::new(move |source_layer: &Arc<Layer>, asset_path: &str| {
            let ctx = ResolveAssetPathContext {
                source_layer: Some(source_layer.clone()),
                asset_path: asset_path.to_string(),
                expression_variables: layer_stack_expr_vars.variables().clone(),
            };
            resolve_asset_path_fn(&ctx)
        });

    // Create anonymous layer
    let tag_str = tag.unwrap_or("");
    let has_extension = !tag_str.is_empty() && tag_str.contains('.');
    let layer_tag = if has_extension {
        tag_str.to_string()
    } else {
        format!("{}.usda", tag_str)
    };

    let output_layer = Layer::create_anonymous(Some(&layer_tag));

    // Flatten fields and specs starting from pseudo root
    let pseudo_root = Path::absolute_root();
    flatten_fields(
        layer_stack,
        &output_layer,
        &pseudo_root,
        SpecType::PseudoRoot,
        &resolve_fn_wrapper,
    );
    flatten_spec_prim(
        layer_stack,
        &output_layer,
        &pseudo_root,
        &resolve_fn_wrapper,
    );

    Some(output_layer)
}

/// Implements the default asset path flattening behavior.
///
/// Matches C++ `UsdFlattenLayerStackResolveAssetPath(const SdfLayerHandle& sourceLayer, const std::string& assetPath)`.
pub fn flatten_layer_stack_resolve_asset_path(
    source_layer: &Arc<Layer>,
    asset_path: &str,
) -> String {
    // Use SdfComputeAssetPathRelativeToLayer equivalent
    usd_sdf::layer_utils::compute_asset_path_relative_to_layer(source_layer, asset_path)
}

/// Implements the default asset path flattening behavior (advanced version).
///
/// Matches C++ `UsdFlattenLayerStackResolveAssetPathAdvanced(const UsdFlattenResolveAssetPathContext& context)`.
pub fn flatten_layer_stack_resolve_asset_path_advanced(
    context: &ResolveAssetPathContext,
) -> String {
    let mut asset_path = &context.asset_path;

    // If the asset path is an expression, compute its value before anchoring
    let evaluated = evaluate_asset_path_expression(asset_path, &context.expression_variables);
    if evaluated != *asset_path {
        asset_path = &evaluated;
    }

    // Anchor asset path relative to source layer
    if let Some(ref source_layer) = context.source_layer {
        flatten_layer_stack_resolve_asset_path(source_layer, asset_path)
    } else {
        asset_path.to_string()
    }
}
