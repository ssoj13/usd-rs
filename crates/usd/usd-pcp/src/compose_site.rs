//! Single-site composition functions.
//!
//! These are helpers that compose specific fields at single sites.
//! They compose the field for a given path across a layer stack,
//! using field-specific rules to combine the values.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/composeSite.h` and `composeSite.cpp`.
//!
//! # Overview
//!
//! These helpers are low-level utilities used by the rest of the PCP algorithms
//! to discover composition arcs in scene description. These arcs are what guide
//! the algorithm to pull additional sites of scene description into the PrimIndex.
//!
//! Some of these field types support list-editing (see `ListOp`). List-editing
//! for these fields is applied across the fixed domain of a single site; you
//! cannot apply list-ops across sites.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::utils::{evaluate_variable_expression, is_variable_expression};
use crate::{
    ErrorType, ExpressionVariables, ExpressionVariablesSource, LayerStackRefPtr, NodeRef,
    Permission,
};
use usd_sdf::{Layer, LayerOffset, ListOp, Path, Payload, Reference, Site as SdfSite};
use usd_vt::Dictionary;

/// Helper information about an arc.
///
/// All arcs have a layer that the arc comes from. References and payloads
/// supply an authored asset path as well.
#[derive(Clone, Default)]
pub struct ArcInfo {
    /// The layer that authored this arc.
    pub source_layer: Option<std::sync::Arc<Layer>>,
    /// The layer offset from the layer stack.
    pub source_layer_stack_offset: LayerOffset,
    /// The authored asset path (for references/payloads).
    pub authored_asset_path: String,
    /// The arc number (index in result list).
    pub arc_num: usize,
}

impl ArcInfo {
    /// Creates a new arc info.
    pub fn new(source_layer: std::sync::Arc<Layer>) -> Self {
        Self {
            source_layer: Some(source_layer),
            ..Default::default()
        }
    }

    /// Sets the source layer stack offset.
    pub fn with_offset(mut self, offset: LayerOffset) -> Self {
        self.source_layer_stack_offset = offset;
        self
    }

    /// Sets the authored asset path.
    pub fn with_asset_path(mut self, path: impl Into<String>) -> Self {
        self.authored_asset_path = path.into();
        self
    }
}

impl std::fmt::Debug for ArcInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcInfo")
            .field(
                "source_layer",
                &self.source_layer.as_ref().map(|l| l.identifier()),
            )
            .field("source_layer_stack_offset", &self.source_layer_stack_offset)
            .field("authored_asset_path", &self.authored_asset_path)
            .field("arc_num", &self.arc_num)
            .finish()
    }
}

/// A vector of reference or payload arc information.
pub type ArcInfoVector = Vec<ArcInfo>;

// ============================================================================
// References
// ============================================================================

/// Compose site references from a layer stack.
///
/// Gathers all reference arcs authored at the given path across all layers
/// in the layer stack, applying list-op semantics.
///
/// # Arguments
///
/// * `layer_stack` - The layer stack to compose from
/// * `path` - The path to query for references
///
/// # Returns
///
/// A tuple of (references, arc_info, errors)
pub fn compose_site_references(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
) -> (Vec<Reference>, ArcInfoVector, Vec<ErrorType>) {
    compose_site_references_with_deps(layer_stack, path, None)
}

/// Compose site references with expression variable dependencies.
///
/// P1-1 FIX: evaluates `${VAR}` expressions in asset paths per C++ composeSite.cpp:101-117.
/// Empty results from expression evaluation are silently skipped (conditional references).
pub fn compose_site_references_with_deps(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    expr_var_deps: Option<&mut HashSet<String>>,
) -> (Vec<Reference>, ArcInfoVector, Vec<ErrorType>) {
    let mut errors = Vec::new();
    let mut result = Vec::new();
    let mut info_map: HashMap<Reference, ArcInfo> = HashMap::new();

    // Build ExpressionVariables from the layer stack's variable map
    let expr_vars = build_expr_vars_from_layer_stack(layer_stack);

    // Iterate layers in reverse (weakest to strongest)
    let layers = layer_stack.get_layers();
    for i in (0..layers.len()).rev() {
        let layer = &layers[i];

        // Get the reference list op from this layer
        if let Some(list_op) = layer.get_reference_list_op(path) {
            let layer_offset = layer_stack.get_layer_offset_at(i);

            // Apply list-op operations
            apply_reference_list_op(
                &list_op,
                &mut result,
                layer.clone(),
                layer_offset,
                &mut info_map,
                Some(&expr_vars),
                path,
                expr_var_deps.is_some(),
                &mut errors,
            );
        }
    }

    // Build info vector in order
    let mut info = Vec::with_capacity(result.len());
    for (i, reference) in result.iter().enumerate() {
        if let Some(mut arc_info) = info_map.remove(reference) {
            arc_info.arc_num = i;
            info.push(arc_info);
        } else {
            info.push(ArcInfo {
                arc_num: i,
                ..Default::default()
            });
        }
    }

    (result, info, errors)
}

/// Compose site references from a node.
pub fn compose_site_references_from_node(
    node: &NodeRef,
) -> (Vec<Reference>, ArcInfoVector, Vec<ErrorType>) {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_references(&layer_stack, &node.path())
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    }
}

/// Build ExpressionVariables from a layer stack's string-keyed variable map.
fn build_expr_vars_from_layer_stack(layer_stack: &LayerStackRefPtr) -> ExpressionVariables {
    let raw = layer_stack.get_expression_variables();
    let source = ExpressionVariablesSource::new();
    let mut dict = Dictionary::new();
    for (k, v) in &raw {
        dict.insert(k.as_str(), v.as_str());
    }
    ExpressionVariables::new(source, dict)
}

/// Apply reference list-op operations to accumulate results.
///
/// P1-1 FIX: if `expr_vars` is provided, evaluates `${VAR}` in asset paths.
fn apply_reference_list_op(
    list_op: &ListOp<Reference>,
    result: &mut Vec<Reference>,
    layer: std::sync::Arc<Layer>,
    layer_offset: Option<LayerOffset>,
    info_map: &mut HashMap<Reference, ArcInfo>,
    expr_vars: Option<&ExpressionVariables>,
    source_path: &Path,
    _track_deps: bool,
    _errors: &mut Vec<ErrorType>,
) {
    let anchor = Some(&layer);

    // Process based on list-op mode
    if list_op.is_explicit() {
        // Explicit mode: replace everything
        result.clear();
        info_map.clear();
        for item in list_op.get_explicit_items() {
            // P1-1: evaluate expression vars in asset path; skip if empty (conditional ref)
            let Some(resolved) = resolve_reference_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            let arc_info = ArcInfo::new(layer.clone())
                .with_asset_path(item.asset_path())
                .with_offset(layer_offset.unwrap_or_default());
            info_map.insert(resolved.clone(), arc_info);
            if !result.contains(&resolved) {
                result.push(resolved);
            }
        }
    } else {
        // Non-explicit mode: apply operations

        // Delete items
        for item in list_op.get_deleted_items() {
            let resolved = resolve_reference(item, layer_offset.as_ref(), anchor);
            result.retain(|r| r != &resolved);
            info_map.remove(&resolved);
        }

        // Add items (append if not present)
        for item in list_op.get_added_items() {
            let Some(resolved) = resolve_reference_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            if !result.contains(&resolved) {
                let arc_info = ArcInfo::new(layer.clone())
                    .with_asset_path(item.asset_path())
                    .with_offset(layer_offset.unwrap_or_default());
                info_map.insert(resolved.clone(), arc_info);
                result.push(resolved);
            }
        }

        // Prepend items — collect first, then splice all at front (matches C++ ApplyOperations)
        let mut prepended = Vec::new();
        for item in list_op.get_prepended_items() {
            let Some(resolved) = resolve_reference_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            result.retain(|r| r != &resolved);
            let arc_info = ArcInfo::new(layer.clone())
                .with_asset_path(item.asset_path())
                .with_offset(layer_offset.unwrap_or_default());
            info_map.insert(resolved.clone(), arc_info);
            if !prepended.contains(&resolved) {
                prepended.push(resolved);
            }
        }

        // Append items — collect first, remove duplicates from result, then append
        let mut appended = Vec::new();
        for item in list_op.get_appended_items() {
            let Some(resolved) = resolve_reference_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            result.retain(|r| r != &resolved);
            prepended.retain(|r| r != &resolved);
            let arc_info = ArcInfo::new(layer.clone())
                .with_asset_path(item.asset_path())
                .with_offset(layer_offset.unwrap_or_default());
            info_map.insert(resolved.clone(), arc_info);
            if !appended.contains(&resolved) {
                appended.push(resolved);
            }
        }

        // Build: prepended + original_remaining + appended
        let mut final_result = prepended;
        final_result.append(result);
        final_result.append(&mut appended);
        *result = final_result;

        // P0-6 FIX: Apply ordered items (C++ _ReorderKeys / SdfListOpTypeOrdered).
        // C++ ApplyOperations calls _ReorderKeys which reorders existing items.
        // Ordered items are *not* added; they only change the order of already-present items.
        let ordered = list_op.get_ordered_items();
        if !ordered.is_empty() {
            apply_reference_ordering(result, ordered, layer_offset.as_ref(), anchor);
        }
    }
}

/// Generic ordered-items reordering per C++ `_ReorderKeysHelper`.
///
/// `resolve` converts an item from the order list into the same form as items in `result`
/// (e.g., to resolve relative paths). Items not in `order` go to the BEGINNING.
fn apply_generic_ordering<T, F>(result: &mut Vec<T>, order: &[T], resolve: F)
where
    T: Clone + PartialEq,
    F: Fn(&T) -> T,
{
    // Resolve and deduplicate order list.
    let mut unique_order: Vec<T> = Vec::new();
    for item in order {
        let resolved = resolve(item);
        if !unique_order.contains(&resolved) {
            unique_order.push(resolved);
        }
    }
    if unique_order.is_empty() {
        return;
    }

    let scratch = std::mem::take(result);
    let n = scratch.len();
    let mut taken = vec![false; n];
    let mut ordered_part: Vec<T> = Vec::with_capacity(n);

    for key in &unique_order {
        if let Some(pos) = scratch.iter().position(|r| r == key) {
            if taken[pos] {
                continue;
            }
            // Extend to next untaken item that is in unique_order.
            let mut end = pos + 1;
            while end < n {
                if !taken[end] && unique_order.contains(&scratch[end]) {
                    break;
                }
                end += 1;
            }
            for i in pos..end {
                if !taken[i] {
                    ordered_part.push(scratch[i].clone());
                    taken[i] = true;
                }
            }
        }
    }

    // Unordered items go to the BEGINNING per C++.
    let mut unordered: Vec<T> = scratch
        .iter()
        .enumerate()
        .filter(|(i, _)| !taken[*i])
        .map(|(_, r)| r.clone())
        .collect();
    unordered.append(&mut ordered_part);
    *result = unordered;
}

/// Apply reference ordering per C++ _ReorderKeysHelper logic.
fn apply_reference_ordering(
    result: &mut Vec<Reference>,
    order: &[Reference],
    layer_offset: Option<&LayerOffset>,
    anchor: Option<&std::sync::Arc<Layer>>,
) {
    // Resolve order items the same way we resolve regular items
    let resolved_order: Vec<Reference> = order
        .iter()
        .map(|r| resolve_reference(r, layer_offset, anchor))
        .collect();

    // Deduplicate order list
    let mut unique_order: Vec<Reference> = Vec::new();
    let mut order_set: Vec<Reference> = Vec::new(); // use Vec for PartialEq since Reference may not be Hash
    for r in &resolved_order {
        if !order_set.contains(r) {
            order_set.push(r.clone());
            unique_order.push(r.clone());
        }
    }
    if unique_order.is_empty() {
        return;
    }

    let scratch = std::mem::take(result);
    let n = scratch.len();
    let mut taken = vec![false; n];

    let mut ordered_part: Vec<Reference> = Vec::with_capacity(n);

    for key in &unique_order {
        // Find key in scratch
        if let Some(pos) = scratch.iter().position(|r| r == key) {
            if taken[pos] {
                continue;
            }
            // Find end: next untaken item that is in order_set
            let mut end = pos + 1;
            while end < n {
                if !taken[end] && order_set.contains(&scratch[end]) {
                    break;
                }
                end += 1;
            }
            for i in pos..end {
                if !taken[i] {
                    ordered_part.push(scratch[i].clone());
                    taken[i] = true;
                }
            }
        }
    }

    // Unordered items go to the BEGINNING per C++ splice-to-begin
    let mut unordered: Vec<Reference> = scratch
        .iter()
        .enumerate()
        .filter(|(i, _)| !taken[*i])
        .map(|(_, r)| r.clone())
        .collect();
    unordered.append(&mut ordered_part);
    *result = unordered;
}

/// Resolve a reference asset path relative to the anchor layer.
///
/// C++ `_CopyCustomData` copies customData from the original reference.
/// We propagate it here so authored metadata on references survives composition.
///
/// P1-1 FIX: evaluates `${VAR}` expressions in asset paths before resolution.
/// Returns None if expression evaluates to empty (conditional reference).
fn resolve_reference_with_expr(
    reference: &Reference,
    layer_offset: Option<&LayerOffset>,
    anchor_layer: Option<&std::sync::Arc<Layer>>,
    expr_vars: Option<&ExpressionVariables>,
    source_path: &Path,
) -> Option<Reference> {
    let mut authored_path = reference.asset_path().to_string();

    // Evaluate expression variables in asset path (C++ Pcp_EvaluateVariableExpression)
    if let Some(ev) = expr_vars {
        if is_variable_expression(&authored_path) {
            let mut expr_errors = Vec::new();
            authored_path = evaluate_variable_expression(
                &authored_path,
                ev,
                "reference",
                None,
                source_path,
                None,
                Some(&mut expr_errors),
            );
            // Empty result = conditional reference, silently skip (C++ returns nullopt)
            if authored_path.is_empty() {
                return None;
            }
        }
    }

    // Resolve relative asset paths using the anchor layer
    let asset_path = if let Some(anchor) = anchor_layer {
        usd_sdf::compute_asset_path_relative_to_layer(anchor, &authored_path)
    } else {
        authored_path
    };

    // Compose layer offsets
    let offset = if let Some(layer_off) = layer_offset {
        reference.layer_offset().compose(layer_off)
    } else {
        *reference.layer_offset()
    };

    // P0-2 FIX: propagate customData from the original reference (C++ _CopyCustomData).
    Some(Reference::with_metadata(
        &asset_path,
        reference.prim_path().as_str(),
        offset,
        reference.custom_data().clone(),
    ))
}

/// Non-expression-evaluating wrapper for backward compat (ordering functions etc).
fn resolve_reference(
    reference: &Reference,
    layer_offset: Option<&LayerOffset>,
    anchor_layer: Option<&std::sync::Arc<Layer>>,
) -> Reference {
    resolve_reference_with_expr(reference, layer_offset, anchor_layer, None, &Path::empty())
        .unwrap_or_else(|| reference.clone())
}

// ============================================================================
// Payloads
// ============================================================================

/// Compose site payloads from a layer stack.
pub fn compose_site_payloads(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
) -> (Vec<Payload>, ArcInfoVector, Vec<ErrorType>) {
    compose_site_payloads_with_deps(layer_stack, path, None)
}

/// Compose site payloads with expression variable dependencies.
///
/// P1-1 FIX: evaluates `${VAR}` expressions in payload asset paths.
pub fn compose_site_payloads_with_deps(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    _expr_var_deps: Option<&mut HashSet<String>>,
) -> (Vec<Payload>, ArcInfoVector, Vec<ErrorType>) {
    let errors = Vec::new();
    let mut result = Vec::new();
    let mut info_map: HashMap<Payload, ArcInfo> = HashMap::new();

    let expr_vars = build_expr_vars_from_layer_stack(layer_stack);

    let layers = layer_stack.get_layers();
    for i in (0..layers.len()).rev() {
        let layer = &layers[i];

        if let Some(list_op) = layer.get_payload_list_op(path) {
            let layer_offset = layer_stack.get_layer_offset_at(i);

            apply_payload_list_op(
                &list_op,
                &mut result,
                layer.clone(),
                layer_offset,
                &mut info_map,
                Some(&expr_vars),
                path,
            );
        }
    }

    // Build info vector
    let mut info = Vec::with_capacity(result.len());
    for (i, payload) in result.iter().enumerate() {
        if let Some(mut arc_info) = info_map.remove(payload) {
            arc_info.arc_num = i;
            info.push(arc_info);
        } else {
            info.push(ArcInfo {
                arc_num: i,
                ..Default::default()
            });
        }
    }

    (result, info, errors)
}

/// Compose site payloads from a node.
pub fn compose_site_payloads_from_node(
    node: &NodeRef,
) -> (Vec<Payload>, ArcInfoVector, Vec<ErrorType>) {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_payloads(&layer_stack, &node.path())
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    }
}

/// Apply payload list-op operations.
///
/// P1-1 FIX: evaluates `${VAR}` expressions in payload asset paths.
fn apply_payload_list_op(
    list_op: &ListOp<Payload>,
    result: &mut Vec<Payload>,
    layer: std::sync::Arc<Layer>,
    layer_offset: Option<LayerOffset>,
    info_map: &mut HashMap<Payload, ArcInfo>,
    expr_vars: Option<&ExpressionVariables>,
    source_path: &Path,
) {
    let anchor = Some(&layer);

    if list_op.is_explicit() {
        result.clear();
        info_map.clear();
        for item in list_op.get_explicit_items() {
            let Some(resolved) = resolve_payload_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            let arc_info = ArcInfo::new(layer.clone())
                .with_asset_path(item.asset_path())
                .with_offset(layer_offset.unwrap_or_default());
            info_map.insert(resolved.clone(), arc_info);
            if !result.contains(&resolved) {
                result.push(resolved);
            }
        }
    } else {
        // Delete
        for item in list_op.get_deleted_items() {
            let resolved = resolve_payload(item, layer_offset.as_ref(), anchor);
            result.retain(|p| p != &resolved);
            info_map.remove(&resolved);
        }

        // Add
        for item in list_op.get_added_items() {
            let Some(resolved) = resolve_payload_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            if !result.contains(&resolved) {
                let arc_info = ArcInfo::new(layer.clone())
                    .with_asset_path(item.asset_path())
                    .with_offset(layer_offset.unwrap_or_default());
                info_map.insert(resolved.clone(), arc_info);
                result.push(resolved);
            }
        }

        // Prepend — collect all first, then splice at front (matches C++ ApplyOperations)
        let mut prepended = Vec::new();
        for item in list_op.get_prepended_items() {
            let Some(resolved) = resolve_payload_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            result.retain(|p| p != &resolved);
            let arc_info = ArcInfo::new(layer.clone())
                .with_asset_path(item.asset_path())
                .with_offset(layer_offset.unwrap_or_default());
            info_map.insert(resolved.clone(), arc_info);
            if !prepended.contains(&resolved) {
                prepended.push(resolved);
            }
        }

        // Append — collect, remove duplicates from result and prepended, then append
        let mut appended = Vec::new();
        for item in list_op.get_appended_items() {
            let Some(resolved) = resolve_payload_with_expr(
                item,
                layer_offset.as_ref(),
                anchor,
                expr_vars,
                source_path,
            ) else {
                continue;
            };
            result.retain(|p| p != &resolved);
            prepended.retain(|p| p != &resolved);
            let arc_info = ArcInfo::new(layer.clone())
                .with_asset_path(item.asset_path())
                .with_offset(layer_offset.unwrap_or_default());
            info_map.insert(resolved.clone(), arc_info);
            if !appended.contains(&resolved) {
                appended.push(resolved);
            }
        }

        // Build: prepended + original_remaining + appended
        let mut final_result = prepended;
        final_result.append(result);
        final_result.append(&mut appended);
        *result = final_result;

        // P0-6 FIX: Apply ordered items reordering (C++ _ReorderKeys).
        let ordered = list_op.get_ordered_items();
        if !ordered.is_empty() {
            apply_generic_ordering(result, ordered, |p| {
                resolve_payload(p, layer_offset.as_ref(), anchor)
            });
        }
    }
}

/// Resolve a payload asset path relative to the anchor layer.
/// Resolve a payload with expression variable evaluation.
///
/// Returns None if expression evaluates to empty (conditional payload).
fn resolve_payload_with_expr(
    payload: &Payload,
    layer_offset: Option<&LayerOffset>,
    anchor_layer: Option<&std::sync::Arc<Layer>>,
    expr_vars: Option<&ExpressionVariables>,
    source_path: &Path,
) -> Option<Payload> {
    let mut authored_path = payload.asset_path().to_string();

    // Evaluate expression variables in asset path
    if let Some(ev) = expr_vars {
        if is_variable_expression(&authored_path) {
            let mut expr_errors = Vec::new();
            authored_path = evaluate_variable_expression(
                &authored_path,
                ev,
                "payload",
                None,
                source_path,
                None,
                Some(&mut expr_errors),
            );
            if authored_path.is_empty() {
                return None;
            }
        }
    }

    let asset_path = if let Some(anchor) = anchor_layer {
        usd_sdf::compute_asset_path_relative_to_layer(anchor, &authored_path)
    } else {
        authored_path
    };

    let offset = if let Some(layer_off) = layer_offset {
        payload.layer_offset().compose(layer_off)
    } else {
        *payload.layer_offset()
    };

    Some(Payload::with_layer_offset(
        &asset_path,
        payload.prim_path().as_str(),
        offset,
    ))
}

/// Non-expression-evaluating wrapper.
fn resolve_payload(
    payload: &Payload,
    layer_offset: Option<&LayerOffset>,
    anchor_layer: Option<&std::sync::Arc<Layer>>,
) -> Payload {
    resolve_payload_with_expr(payload, layer_offset, anchor_layer, None, &Path::empty())
        .unwrap_or_else(|| payload.clone())
}

// ============================================================================
// Inherits
// ============================================================================

/// Compose site inherits from a layer stack.
pub fn compose_site_inherits(layer_stack: &LayerStackRefPtr, path: &Path) -> Vec<Path> {
    let (result, _) = compose_site_inherits_with_info(layer_stack, path);
    result
}

/// Compose site inherits with arc info.
pub fn compose_site_inherits_with_info(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
) -> (Vec<Path>, ArcInfoVector) {
    let mut result = Vec::new();
    let mut info_map: HashMap<Path, ArcInfo> = HashMap::new();

    let layers = layer_stack.get_layers();
    for i in (0..layers.len()).rev() {
        let layer = &layers[i];

        if let Some(list_op) = layer.get_inherit_paths_list_op(path) {
            apply_path_list_op(&list_op, &mut result, layer.clone(), &mut info_map);
        }
    }

    // Build info vector
    let mut info = Vec::with_capacity(result.len());
    for (i, inherit_path) in result.iter().enumerate() {
        if let Some(mut arc_info) = info_map.remove(inherit_path) {
            arc_info.arc_num = i;
            info.push(arc_info);
        } else {
            info.push(ArcInfo {
                arc_num: i,
                ..Default::default()
            });
        }
    }

    (result, info)
}

/// Compose site inherits from a node.
pub fn compose_site_inherits_from_node(node: &NodeRef) -> Vec<Path> {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_inherits(&layer_stack, &node.path())
    } else {
        Vec::new()
    }
}

// ============================================================================
// Specializes
// ============================================================================

/// Compose site specializes from a layer stack.
pub fn compose_site_specializes(layer_stack: &LayerStackRefPtr, path: &Path) -> Vec<Path> {
    let (result, _) = compose_site_specializes_with_info(layer_stack, path);
    result
}

/// Compose site specializes with arc info.
pub fn compose_site_specializes_with_info(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
) -> (Vec<Path>, ArcInfoVector) {
    let mut result = Vec::new();
    let mut info_map: HashMap<Path, ArcInfo> = HashMap::new();

    let layers = layer_stack.get_layers();
    for i in (0..layers.len()).rev() {
        let layer = &layers[i];

        if let Some(list_op) = layer.get_specializes_list_op(path) {
            apply_path_list_op(&list_op, &mut result, layer.clone(), &mut info_map);
        }
    }

    // Build info vector
    let mut info = Vec::with_capacity(result.len());
    for (i, spec_path) in result.iter().enumerate() {
        if let Some(mut arc_info) = info_map.remove(spec_path) {
            arc_info.arc_num = i;
            info.push(arc_info);
        } else {
            info.push(ArcInfo {
                arc_num: i,
                ..Default::default()
            });
        }
    }

    (result, info)
}

/// Compose site specializes from a node.
pub fn compose_site_specializes_from_node(node: &NodeRef) -> Vec<Path> {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_specializes(&layer_stack, &node.path())
    } else {
        Vec::new()
    }
}

/// Apply path list-op operations.
fn apply_path_list_op(
    list_op: &ListOp<Path>,
    result: &mut Vec<Path>,
    layer: std::sync::Arc<Layer>,
    info_map: &mut HashMap<Path, ArcInfo>,
) {
    if list_op.is_explicit() {
        result.clear();
        info_map.clear();
        for item in list_op.get_explicit_items() {
            info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
            if !result.contains(item) {
                result.push(item.clone());
            }
        }
    } else {
        // Delete
        for item in list_op.get_deleted_items() {
            result.retain(|p| p != item);
            info_map.remove(item);
        }

        // Add
        for item in list_op.get_added_items() {
            if !result.contains(item) {
                info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
                result.push(item.clone());
            }
        }

        // Prepend — collect first, splice at front
        let mut prepended = Vec::new();
        for item in list_op.get_prepended_items() {
            result.retain(|p| p != item);
            info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
            if !prepended.contains(item) {
                prepended.push(item.clone());
            }
        }

        // Append — remove from result/prepended first, then append
        let mut appended = Vec::new();
        for item in list_op.get_appended_items() {
            result.retain(|p| p != item);
            prepended.retain(|p| p != item);
            info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
            if !appended.contains(item) {
                appended.push(item.clone());
            }
        }

        // Build: prepended + original_remaining + appended
        let mut final_result = prepended;
        final_result.append(result);
        final_result.append(&mut appended);
        *result = final_result;

        // P0-6 FIX: Apply ordered items reordering (C++ _ReorderKeys).
        let ordered = list_op.get_ordered_items();
        if !ordered.is_empty() {
            apply_generic_ordering(result, ordered, |p| p.clone());
        }
    }
}

// ============================================================================
// Variant Sets
// ============================================================================

/// Compose site variant sets from a layer stack.
pub fn compose_site_variant_sets(layer_stack: &LayerStackRefPtr, path: &Path) -> Vec<String> {
    let (result, _) = compose_site_variant_sets_with_info(layer_stack, path);
    result
}

/// Compose site variant sets with arc info.
pub fn compose_site_variant_sets_with_info(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
) -> (Vec<String>, ArcInfoVector) {
    let mut result = Vec::new();
    let mut info_map: HashMap<String, ArcInfo> = HashMap::new();

    let layers = layer_stack.get_layers();
    for i in (0..layers.len()).rev() {
        let layer = &layers[i];

        if let Some(list_op) = layer.get_variant_set_names_list_op(path) {
            apply_string_list_op(&list_op, &mut result, layer.clone(), &mut info_map);
        }
    }

    // Build info vector
    let mut info = Vec::with_capacity(result.len());
    for (i, vset_name) in result.iter().enumerate() {
        if let Some(mut arc_info) = info_map.remove(vset_name) {
            arc_info.arc_num = i;
            info.push(arc_info);
        } else {
            info.push(ArcInfo {
                arc_num: i,
                ..Default::default()
            });
        }
    }

    (result, info)
}

/// Compose site variant sets from a node.
pub fn compose_site_variant_sets_from_node(node: &NodeRef) -> Vec<String> {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_variant_sets(&layer_stack, &node.path())
    } else {
        Vec::new()
    }
}

/// Apply string list-op operations.
fn apply_string_list_op(
    list_op: &ListOp<String>,
    result: &mut Vec<String>,
    layer: std::sync::Arc<Layer>,
    info_map: &mut HashMap<String, ArcInfo>,
) {
    if list_op.is_explicit() {
        result.clear();
        info_map.clear();
        for item in list_op.get_explicit_items() {
            info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
            if !result.contains(item) {
                result.push(item.clone());
            }
        }
    } else {
        for item in list_op.get_deleted_items() {
            result.retain(|s| s != item);
            info_map.remove(item);
        }

        for item in list_op.get_added_items() {
            if !result.contains(item) {
                info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
                result.push(item.clone());
            }
        }

        // Prepend — collect first, splice at front
        let mut prepended = Vec::new();
        for item in list_op.get_prepended_items() {
            result.retain(|s| s != item);
            info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
            if !prepended.contains(item) {
                prepended.push(item.clone());
            }
        }

        // Append — remove from result/prepended first, then append
        let mut appended = Vec::new();
        for item in list_op.get_appended_items() {
            result.retain(|s| s != item);
            prepended.retain(|s| s != item);
            info_map.insert(item.clone(), ArcInfo::new(layer.clone()));
            if !appended.contains(item) {
                appended.push(item.clone());
            }
        }

        // Build: prepended + original_remaining + appended
        let mut final_result = prepended;
        final_result.append(result);
        final_result.append(&mut appended);
        *result = final_result;

        // P0-6 FIX: Apply ordered items reordering (C++ _ReorderKeys).
        let ordered = list_op.get_ordered_items();
        if !ordered.is_empty() {
            apply_generic_ordering(result, ordered, |s| s.clone());
        }
    }
}

// ============================================================================
// Variant Selection
// ============================================================================

/// A map of variant set name to selected variant.
pub type VariantSelectionMap = HashMap<String, String>;

/// Compose a single variant selection from a layer stack.
///
/// P1-2 FIX: evaluates `${VAR}` expressions in variant selections.
/// Selections that produce errors are skipped (fall through to weaker opinion).
pub fn compose_site_variant_selection(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    vset_name: &str,
) -> Option<String> {
    compose_site_variant_selection_with_deps(layer_stack, path, vset_name, None, None)
}

/// Compose a single variant selection with expression variable dependency tracking.
pub fn compose_site_variant_selection_with_deps(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    vset_name: &str,
    mut expr_var_deps: Option<&mut HashSet<String>>,
    _errors: Option<&mut Vec<ErrorType>>,
) -> Option<String> {
    let expr_vars = build_expr_vars_from_layer_stack(layer_stack);
    let layers = layer_stack.get_layers();
    for layer in &layers {
        if let Some(mut selection) = layer.get_variant_selection(path, vset_name) {
            // P1-2: evaluate expression variables in variant selection
            if is_variable_expression(&selection) {
                let mut expr_errors = Vec::new();
                selection = evaluate_variable_expression(
                    &selection,
                    &expr_vars,
                    "variant",
                    None,
                    path,
                    expr_var_deps.as_deref_mut(),
                    Some(&mut expr_errors),
                );
                // On error, skip this selection and fall through to weaker opinion (C++ behavior)
                if !expr_errors.is_empty() {
                    continue;
                }
            }
            return Some(selection);
        }
    }
    None
}

/// Compose all variant selections from a layer stack.
///
/// P1-3 FIX: evaluates `${VAR}` expressions in variant selections.
/// Erroneous selections are removed per C++ composeSite.cpp:444-465.
pub fn compose_site_variant_selections(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
) -> VariantSelectionMap {
    compose_site_variant_selections_with_deps(layer_stack, path, None, None)
}

/// Compose all variant selections with expression variable dependency tracking.
pub fn compose_site_variant_selections_with_deps(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    mut expr_var_deps: Option<&mut HashSet<String>>,
    mut errors: Option<&mut Vec<ErrorType>>,
) -> VariantSelectionMap {
    let mut result = VariantSelectionMap::new();
    let expr_vars = build_expr_vars_from_layer_stack(layer_stack);

    let layers = layer_stack.get_layers();
    for layer in &layers {
        if let Some(selections) = layer.get_variant_selections(path) {
            for (key, mut value) in selections {
                // Evaluate ${VAR} expressions in variant selections; propagate
                // used-variable deps and errors to callers so they can track
                // which expression variables influenced composition results.
                if is_variable_expression(&value) {
                    let mut expr_errors = Vec::new();
                    value = evaluate_variable_expression(
                        &value,
                        &expr_vars,
                        "variant",
                        None,
                        path,
                        expr_var_deps.as_deref_mut(),
                        Some(&mut expr_errors),
                    );
                    // On error, skip this selection (C++ erases from vselMap)
                    if !expr_errors.is_empty() {
                        // Propagate error discriminators to caller.
                        if let Some(ref mut errs) = errors {
                            for _ in &expr_errors {
                                errs.push(ErrorType::VariableExpressionError);
                            }
                        }
                        continue;
                    }
                }
                // Only insert if not already present (stronger opinion wins)
                result.entry(key).or_insert(value);
            }
        }
    }

    result
}

/// Check if a layer stack has any variant selections at a path.
pub fn compose_site_has_variant_selections(layer_stack: &LayerStackRefPtr, path: &Path) -> bool {
    let layers = layer_stack.get_layers();
    for layer in &layers {
        if layer.has_variant_selections(path) {
            return true;
        }
    }
    false
}

// ============================================================================
// Variant Options
// ============================================================================

/// Compose variant set options (available variants in a set).
pub fn compose_site_variant_set_options(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    vset_name: &str,
) -> HashSet<String> {
    let mut result = HashSet::new();

    // Build the variant set path
    if let Some(vset_path) = path.append_variant_selection(vset_name, "") {
        let layers = layer_stack.get_layers();
        for layer in &layers {
            if let Some(children) = layer.get_variant_children(&vset_path) {
                for child in children {
                    result.insert(child);
                }
            }
        }
    }

    result
}

// ============================================================================
// Permission
// ============================================================================

/// Compose site permission from a layer stack.
pub fn compose_site_permission(layer_stack: &LayerStackRefPtr, path: &Path) -> Permission {
    let layers = layer_stack.get_layers();
    for layer in &layers {
        if let Some(perm) = layer.get_permission(path) {
            return perm;
        }
    }
    Permission::Public
}

/// Compose site permission from a node.
pub fn compose_site_permission_from_node(node: &NodeRef) -> Permission {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_permission(&layer_stack, &node.path())
    } else {
        Permission::Public
    }
}

// ============================================================================
// Has Specs
// ============================================================================

/// Check if a layer stack has any specs at a path.
pub fn compose_site_has_specs(layer_stack: &LayerStackRefPtr, path: &Path) -> bool {
    let layers = layer_stack.get_layers();
    for layer in &layers {
        if layer.has_spec(path) {
            return true;
        }
    }
    false
}

/// Check if a layer stack has any specs at a path, ignoring certain layers.
///
/// Takes a slice of layer identifiers to ignore.
pub fn compose_site_has_specs_ignoring(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    layers_to_ignore: &[String],
) -> bool {
    let layers = layer_stack.get_layers();
    for layer in &layers {
        let should_ignore = layers_to_ignore.iter().any(|id| layer.identifier() == id);
        if !should_ignore && layer.has_spec(path) {
            return true;
        }
    }
    false
}

/// Check if a layer stack has any specs at a path, ignoring certain layers.
///
/// Takes a set of layer handles to ignore.
/// Matches C++ `PcpComposeSiteHasSpecs()` with `layersToIgnore` parameter.
pub fn compose_site_has_specs_ignoring_layers(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    layers_to_ignore: &HashSet<Arc<Layer>>,
) -> bool {
    let layers = layer_stack.get_layers();
    for layer in &layers {
        let should_ignore = layers_to_ignore
            .iter()
            .any(|ignored| Arc::ptr_eq(ignored, layer));
        if !should_ignore && layer.has_spec(path) {
            return true;
        }
    }
    false
}

/// Check if a layer stack has specs from a node.
pub fn compose_site_has_specs_from_node(node: &NodeRef) -> bool {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_has_specs(&layer_stack, &node.path())
    } else {
        false
    }
}

// ============================================================================
// Prim Sites
// ============================================================================

/// Compose prim sites from a layer stack.
pub fn compose_site_prim_sites(layer_stack: &LayerStackRefPtr, path: &Path) -> Vec<SdfSite> {
    use usd_sdf::LayerHandle;

    let mut result = Vec::new();
    let layers = layer_stack.get_layers();

    for layer in &layers {
        if layer.has_spec(path) {
            result.push(SdfSite {
                layer: LayerHandle::from_layer(layer),
                path: path.clone(),
            });
        }
    }

    result
}

/// Compose prim sites from a node.
pub fn compose_site_prim_sites_from_node(node: &NodeRef) -> Vec<SdfSite> {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_prim_sites(&layer_stack, &node.path())
    } else {
        Vec::new()
    }
}

// ============================================================================
// Symmetry
// ============================================================================

/// Returns true if the site has symmetry metadata.
///
/// Matches C++ `PcpComposeSiteHasSymmetry()`.
pub fn compose_site_has_symmetry(layer_stack: &LayerStackRefPtr, path: &Path) -> bool {
    use usd_tf::Token;

    let layers = layer_stack.get_layers();

    // Check for symmetry metadata in any layer
    for layer in &layers {
        // Check for SymmetryFunction or SymmetryArguments fields
        if layer.has_field(path, &Token::new("symmetryFunction"))
            || layer.has_field(path, &Token::new("symmetryArguments"))
        {
            return true;
        }
    }

    false
}

/// Returns true if the node has symmetry metadata.
pub fn compose_site_has_symmetry_from_node(node: &NodeRef) -> bool {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_has_symmetry(&layer_stack, &node.path())
    } else {
        false
    }
}

// ============================================================================
// Value Clips
// ============================================================================

/// Returns true if the site has value clips metadata.
///
/// Matches C++ `PcpComposeSiteHasValueClips()`.
pub fn compose_site_has_value_clips(layer_stack: &LayerStackRefPtr, path: &Path) -> bool {
    use usd_tf::Token;

    let layers = layer_stack.get_layers();

    // Check for value clips metadata in any layer
    for layer in &layers {
        // Check for Clips field
        if layer.has_field(path, &Token::new("clips")) {
            return true;
        }
    }

    false
}

/// Returns true if the node has value clips metadata.
pub fn compose_site_has_value_clips_from_node(node: &NodeRef) -> bool {
    if let Some(layer_stack) = node.layer_stack() {
        compose_site_has_value_clips(&layer_stack, &node.path())
    } else {
        false
    }
}

// ============================================================================
// Deprecated Functions
// ============================================================================

/// \deprecated Use `compose_site_has_specs()` instead.
///
/// Matches C++ `PcpComposeSiteHasPrimSpecs()` (deprecated).
#[deprecated(note = "Use compose_site_has_specs instead")]
pub fn compose_site_has_prim_specs(layer_stack: &LayerStackRefPtr, path: &Path) -> bool {
    compose_site_has_specs(layer_stack, path)
}

/// \deprecated Use `compose_site_has_specs_ignoring_layers()` instead.
#[deprecated(note = "Use compose_site_has_specs_ignoring_layers instead")]
pub fn compose_site_has_prim_specs_ignoring(
    layer_stack: &LayerStackRefPtr,
    path: &Path,
    layers_to_ignore: &HashSet<std::sync::Arc<Layer>>,
) -> bool {
    compose_site_has_specs_ignoring_layers(layer_stack, path, layers_to_ignore)
}

/// \deprecated Use `compose_site_has_specs_from_node()` instead.
#[deprecated(note = "Use compose_site_has_specs_from_node instead")]
pub fn compose_site_has_prim_specs_from_node(node: &NodeRef) -> bool {
    compose_site_has_specs_from_node(node)
}

// ============================================================================
// Child Names
// ============================================================================

use usd_tf::Token;

/// Token set for efficient lookup.
pub type TokenSet = HashSet<Token>;

/// Compose child names from layers.
///
/// # Arguments
///
/// * `layers` - The layers to compose from
/// * `path` - The path to query
/// * `names_field` - The field containing child names
/// * `order_field` - Optional field containing ordering
///
/// # Returns
///
/// A tuple of (ordered names, name set)
pub fn compose_site_child_names(
    layers: &[std::sync::Arc<Layer>],
    path: &Path,
    names_field: &Token,
    order_field: Option<&Token>,
) -> (Vec<Token>, TokenSet) {
    let mut name_order = Vec::new();
    let mut name_set = TokenSet::new();

    // Iterate weakest to strongest (TF_REVERSE_FOR_ALL on a strongest-first list).
    // C++ PcpComposeSiteChildNames uses TF_REVERSE_FOR_ALL: the weakest layer's
    // names appear first in nameOrder; stronger layers then append names not yet
    // seen. The final ordering is corrected by orderField (SdfApplyListOrdering).
    for layer in layers.iter().rev() {
        // Get names from this layer
        if let Some(names) = layer.get_field_as_token_vector(path, names_field) {
            if name_set.is_empty() {
                // Optimization: if name_set is empty, insert all at once
                name_set.extend(names.iter().cloned());
                if name_set.len() == names.len() {
                    // All unique, just use the names directly
                    name_order = names;
                } else {
                    // Duplicates found, do one-by-one
                    name_set.clear();
                    for name in names {
                        if name_set.insert(name.clone()) {
                            name_order.push(name);
                        }
                    }
                }
            } else {
                // Add names that aren't already present
                for name in names {
                    if name_set.insert(name.clone()) {
                        name_order.push(name);
                    }
                }
            }
        }

        // Apply ordering if specified
        if let Some(order_token) = order_field {
            if let Some(order) = layer.get_field_as_token_vector(path, order_token) {
                apply_list_ordering(&mut name_order, &order);
            }
        }
    }

    (name_order, name_set)
}

/// Apply list ordering using the C++ `SdfApplyListOrdering` / `_ReorderKeysHelper` algorithm.
///
/// C++ uses a splice-based approach:
/// 1. For each key in `order` (deduplicated), find the next item in scratch that is
///    also in the order set; move that contiguous sub-range to result.
/// 2. Whatever remains in scratch (unordered items) is spliced to the BEGINNING of result.
///
/// This differs from a naive sort: unordered items stay first, not last.
pub(crate) fn apply_list_ordering(items: &mut Vec<Token>, order: &[Token]) {
    if order.is_empty() || items.is_empty() {
        return;
    }

    // Build deduplicated order vector and order set.
    let mut unique_order: Vec<Token> = Vec::new();
    let mut order_set: HashSet<Token> = HashSet::new();
    for t in order {
        if order_set.insert(t.clone()) {
            unique_order.push(t.clone());
        }
    }
    if unique_order.is_empty() {
        return;
    }

    // Build index map: token -> position in scratch for fast lookup.
    let scratch: Vec<Token> = std::mem::take(items);
    // taken[i] = true means scratch[i] has been moved to result already.
    let mut taken = vec![false; scratch.len()];
    // Map each token value to its first index in scratch.
    let mut search: HashMap<Token, usize> = HashMap::new();
    for (i, t) in scratch.iter().enumerate() {
        search.entry(t.clone()).or_insert(i);
    }

    let mut result: Vec<Token> = Vec::with_capacity(scratch.len());

    // For each key in unique_order, find it in scratch, then extend to the next
    // order-set member — move that entire sub-range to result.
    for key in &unique_order {
        if let Some(&pos) = search.get(key) {
            if taken[pos] {
                continue;
            }
            // Find the end of the contiguous run: advance until we hit another
            // order-set member (which will be handled in a later iteration).
            let mut end = pos + 1;
            while end < scratch.len() {
                if !taken[end] && order_set.contains(&scratch[end]) {
                    break;
                }
                end += 1;
            }
            // Move [pos..end) skipping already-taken slots.
            for i in pos..end {
                if !taken[i] {
                    result.push(scratch[i].clone());
                    taken[i] = true;
                }
            }
        }
    }

    // Any items remaining in scratch (not in order at all) go to the BEGINNING — matching C++.
    let mut unordered: Vec<Token> = scratch
        .iter()
        .enumerate()
        .filter(|(i, _)| !taken[*i])
        .map(|(_, t)| t.clone())
        .collect();
    unordered.append(&mut result);
    *items = unordered;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayerStackIdentifier;

    #[test]
    fn test_arc_info_default() {
        let info = ArcInfo::default();
        assert!(info.source_layer.is_none());
        assert_eq!(info.arc_num, 0);
        assert!(info.authored_asset_path.is_empty());
    }

    #[test]
    fn test_arc_info_builder() {
        let layer = Layer::create_anonymous(None);
        let info = ArcInfo::new(layer)
            .with_asset_path("./model.usda")
            .with_offset(LayerOffset::new(10.0, 2.0));

        assert!(info.source_layer.is_some());
        assert_eq!(info.authored_asset_path, "./model.usda");
        assert_eq!(info.source_layer_stack_offset.offset(), 10.0);
    }

    #[test]
    fn test_apply_list_ordering() {
        // P0-1 FIX verification: C++ SdfApplyListOrdering splice-based algorithm.
        // items = [c, a, b, d], order = [a, b, c]
        //
        // C++ algorithm:
        //   key=a: pos=1, next order item=b at pos=2 → run=[a]. ordered_part=[a]
        //   key=b: pos=2, next order item=c... but d at pos=3 is NOT in order_set → d gets included.
        //          Run=[b,d]. ordered_part=[a,b,d]
        //   key=c: pos=0 (not taken), run=[c]. ordered_part=[a,b,d,c]
        //   unordered=[] (all taken), final=[a,b,d,c]
        let mut items = vec![
            Token::from("c"),
            Token::from("a"),
            Token::from("b"),
            Token::from("d"),
        ];
        let order = vec![Token::from("a"), Token::from("b"), Token::from("c")];

        apply_list_ordering(&mut items, &order);

        // d is adjacent to b (not in order_set), so it gets swept along with b's run.
        assert_eq!(items[0].as_str(), "a");
        assert_eq!(items[1].as_str(), "b");
        assert_eq!(items[2].as_str(), "d");
        assert_eq!(items[3].as_str(), "c");
    }

    #[test]
    fn test_apply_list_ordering_all_ordered() {
        // items = [X, Y, A, B], order = [A, B] — unordered items (X,Y) at beginning
        let mut items = vec![
            Token::from("X"),
            Token::from("Y"),
            Token::from("A"),
            Token::from("B"),
        ];
        let order = vec![Token::from("A"), Token::from("B")];
        apply_list_ordering(&mut items, &order);

        // X, Y unordered — stay at front; A, B ordered
        assert_eq!(items[0].as_str(), "X");
        assert_eq!(items[1].as_str(), "Y");
        assert_eq!(items[2].as_str(), "A");
        assert_eq!(items[3].as_str(), "B");
    }

    #[test]
    fn test_apply_list_ordering_interleaved() {
        // items = [C, A, B, D], order = [A, B, C]
        // C++ splice algorithm:
        //   key=A: pos=1, next order item=B at pos=2 → run=[A]. ordered_part=[A]
        //   key=B: pos=2, next non-taken order item=C at pos=0 (already earlier in scratch),
        //          but searching forward: end=3(D, not in order_set)→end=4. run=[B,D]. ordered=[A,B,D]
        //   key=C: pos=0 (not taken), end=1(A taken)→2(B taken)→3(D taken)→4. run=[C]. ordered=[A,B,D,C]
        //   unordered=[] (all taken), final=[A,B,D,C]
        let mut items = vec![
            Token::from("C"),
            Token::from("A"),
            Token::from("B"),
            Token::from("D"),
        ];
        let order = vec![Token::from("A"), Token::from("B"), Token::from("C")];
        apply_list_ordering(&mut items, &order);

        assert_eq!(items[0].as_str(), "A");
        assert_eq!(items[1].as_str(), "B");
        assert_eq!(items[2].as_str(), "D");
        assert_eq!(items[3].as_str(), "C");
    }

    #[test]
    fn test_variant_selection_map() {
        let mut selections = VariantSelectionMap::new();
        selections.insert("shading".to_string(), "full".to_string());
        selections.insert("lod".to_string(), "high".to_string());

        assert_eq!(selections.get("shading"), Some(&"full".to_string()));
        assert_eq!(selections.get("lod"), Some(&"high".to_string()));
        assert!(selections.get("nonexistent").is_none());
    }

    #[test]
    fn test_compose_empty_layer_stack() {
        let id = LayerStackIdentifier::new("test.usda");
        let layer_stack = crate::LayerStack::new(id);
        let path = Path::from_string("/World").unwrap();

        // These should all return empty results for empty layer stack
        let (refs, _, _) = compose_site_references(&layer_stack, &path);
        assert!(refs.is_empty());

        let (payloads, _, _) = compose_site_payloads(&layer_stack, &path);
        assert!(payloads.is_empty());

        let inherits = compose_site_inherits(&layer_stack, &path);
        assert!(inherits.is_empty());

        let specializes = compose_site_specializes(&layer_stack, &path);
        assert!(specializes.is_empty());

        let vsets = compose_site_variant_sets(&layer_stack, &path);
        assert!(vsets.is_empty());
    }

    #[test]
    fn test_compose_site_permission_default() {
        let id = LayerStackIdentifier::new("test.usda");
        let layer_stack = crate::LayerStack::new(id);
        let path = Path::from_string("/World").unwrap();

        // Default permission is Public
        let perm = compose_site_permission(&layer_stack, &path);
        assert_eq!(perm, Permission::Public);
    }

    #[test]
    fn test_compose_site_has_specs_empty() {
        let id = LayerStackIdentifier::new("test.usda");
        let layer_stack = crate::LayerStack::new(id);
        let path = Path::from_string("/World").unwrap();

        assert!(!compose_site_has_specs(&layer_stack, &path));
    }

    #[test]
    fn test_compose_site_references_with_layer() {
        // Create a layer with a reference on a prim
        let layer = Layer::create_anonymous(None);
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, usd_sdf::Specifier::Def, "");

        // Add a reference via set_field with a ReferenceListOp
        let reference = Reference::new(
            "./model.usda".to_string(),
            Path::from_string("/Model").unwrap(),
        );
        let mut list_op = usd_sdf::ReferenceListOp::default();
        let _ = list_op.set_explicit_items(vec![reference]);
        layer.set_field(
            &prim_path,
            &usd_tf::Token::new("references"),
            usd_vt::Value::from(list_op),
        );

        // Build a layer stack containing this layer
        let stack = crate::LayerStack::from_root_layer(layer);

        // Compose references
        let (refs, _arc_info, _errors) = compose_site_references(&stack, &prim_path);
        // The reference should be present
        assert!(!refs.is_empty());
    }
}
