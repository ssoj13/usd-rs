//! Layer stack flattening utilities.
//!
//! Provides utilities for flattening a USD stage's layer stack into a single layer.

use std::sync::Arc;
use usd_core::stage::Stage;
use usd_sdf::layer::Layer;
use usd_sdf::layer_offset::LayerOffset;

use crate::authoring;

/// Callback function for resolving asset paths during flattening.
///
/// The callback receives the source layer and the asset path authored in that
/// layer. It should return the string that should be authored in the flattened
/// layer.
pub type ResolveAssetPathFn = dyn Fn(&Arc<Layer>, &str) -> String + Send + Sync;

/// A boxed resolve asset path function.
pub type BoxedResolveAssetPathFn = Box<ResolveAssetPathFn>;

/// Flattens the root layer stack of a stage into a single layer.
///
/// The result layer can be substituted for the original layer stack while
/// producing the same composed UsdStage.
///
/// Unlike `Stage::export()`, this function does not flatten composition arcs
/// such as references, payloads, inherits, specializes, or variants.
///
/// Sublayer time offsets will be applied to remap any time-keyed scene
/// description (timeSamples and clips).
///
/// Asset paths will be resolved to absolute form to ensure they continue
/// to identify the same asset from the output layer.
///
/// # Arguments
///
/// * `stage` - The stage to flatten
/// * `tag` - Optional tag for the resulting layer
///
/// # Returns
///
/// The flattened layer, or `None` on error.
///
/// # Notes
///
/// The SdfListOp operations "add" and "reorder" cannot be flattened into a
/// single opinion. "add" will be converted to "append", and "reorder" will
/// be discarded.
pub fn flatten_layer_stack(stage: &Arc<Stage>, tag: Option<&str>) -> Option<Arc<Layer>> {
    flatten_layer_stack_with_resolver(stage, None, tag)
}

/// Flattens the layer stack with a custom asset path resolver.
///
/// This is an advanced version that accepts a callback to customize how
/// asset paths are resolved.
pub fn flatten_layer_stack_with_resolver(
    stage: &Arc<Stage>,
    resolve_asset_path_fn: Option<&BoxedResolveAssetPathFn>,
    tag: Option<&str>,
) -> Option<Arc<Layer>> {
    // Create the result layer
    let result_layer = Layer::create_anonymous(tag.or(Some("flattened")));

    // Get the layer stack
    let root_layer = stage.get_root_layer();
    let session_layer = stage.get_session_layer();

    // Copy root layer metadata (delegate to authoring::copy_layer_metadata)
    authoring::copy_layer_metadata(&root_layer, &result_layer, false, false);

    // Get all sublayers in strength order
    let sublayers = get_sublayers_in_strength_order(&root_layer);

    // Flatten each sublayer into the result
    for (sublayer, offset) in sublayers {
        flatten_sublayer_into(&sublayer, &result_layer, offset, resolve_asset_path_fn);
    }

    // If there's a session layer, flatten it too (it's strongest)
    if let Some(session) = session_layer {
        flatten_sublayer_into(
            &session,
            &result_layer,
            LayerOffset::default(),
            resolve_asset_path_fn,
        );
    }

    Some(result_layer)
}

/// Default asset path resolver for layer stack flattening.
///
/// For paths that the current ArResolver identifies as search paths or
/// absolute paths, returns the unmodified path. Any "layer relative path"
/// will be absolutized.
pub fn flatten_layer_stack_resolve_asset_path(
    source_layer: &Arc<Layer>,
    asset_path: &str,
) -> String {
    if asset_path.is_empty() {
        return String::new();
    }

    // If the path is already absolute or a search path, return as-is
    if asset_path.starts_with('/') || asset_path.contains("://") {
        return asset_path.to_string();
    }

    // On Windows, check for drive letter paths
    if asset_path.len() >= 2 && asset_path.as_bytes()[1] == b':' {
        return asset_path.to_string();
    }

    // For relative paths, make them absolute relative to the source layer
    if let Some(layer_path) = source_layer.real_path() {
        if let Some(parent) = layer_path.parent() {
            let absolute = parent.join(asset_path);
            if let Some(s) = absolute.to_str() {
                return s.to_string();
            }
        }
    }

    asset_path.to_string()
}

// copy_layer_metadata removed — now delegates to authoring::copy_layer_metadata

/// Gets all sublayers in strength order with their offsets.
fn get_sublayers_in_strength_order(layer: &Arc<Layer>) -> Vec<(Arc<Layer>, LayerOffset)> {
    let mut result = Vec::new();

    // The root layer itself comes first
    result.push((Arc::clone(layer), LayerOffset::default()));

    // Then process sublayers recursively (in order from first to last,
    // where first is strongest)
    let sublayer_paths = layer.get_sublayer_paths();
    let offsets = layer.get_sublayer_offsets();

    for (i, path) in sublayer_paths.iter().enumerate() {
        // Try to open the sublayer
        if let Ok(sublayer) = Layer::find_or_open(path) {
            // Get the offset for this sublayer
            let offset = offsets.get(i).cloned().unwrap_or_default();

            // Recursively get nested sublayers
            let nested = get_sublayers_in_strength_order(&sublayer);

            for (nested_layer, nested_offset) in nested {
                // Compose the offsets
                let composed = offset * nested_offset;
                result.push((nested_layer, composed));
            }
        }
    }

    result
}

/// Flattens a sublayer into the result layer.
fn flatten_sublayer_into(
    source: &Arc<Layer>,
    destination: &Arc<Layer>,
    offset: LayerOffset,
    resolve_asset_path_fn: Option<&BoxedResolveAssetPathFn>,
) {
    // Get root prims from source layer
    let root_prims = source.root_prims();

    // Flatten each root prim into destination
    for prim in root_prims {
        flatten_prim_spec_into(source, &prim, destination, offset, resolve_asset_path_fn);
    }
}

/// Recursively flattens a prim spec into the destination layer.
fn flatten_prim_spec_into(
    source_layer: &Arc<Layer>,
    source_spec: &usd_sdf::prim_spec::PrimSpec,
    destination: &Arc<Layer>,
    offset: LayerOffset,
    resolve_asset_path_fn: Option<&BoxedResolveAssetPathFn>,
) {
    use usd_sdf::prim_spec::PrimSpec;

    let prim_path = source_spec.path();

    // Get or create the corresponding spec in destination
    let mut dest_spec = if let Some(existing) = destination.get_prim_at_path(&prim_path) {
        existing
    } else {
        // Create new prim spec at path
        let layer_handle = destination.get_handle();
        let specifier = source_spec.specifier();
        let type_name = source_spec.type_name();
        match PrimSpec::new_root(
            &layer_handle,
            prim_path.get_name(),
            specifier,
            type_name.as_str(),
        ) {
            Ok(spec) => spec,
            Err(_) => return,
        }
    };

    // Copy spec type
    dest_spec.set_specifier(source_spec.specifier());

    // Copy type name
    let type_name = source_spec.type_name();
    if !type_name.is_empty() {
        dest_spec.set_type_name(type_name.as_str());
    }

    // Copy metadata
    copy_prim_metadata(source_spec, &mut dest_spec);

    // Copy properties
    for property in source_spec.properties() {
        if let Some(attr) = property.as_attribute() {
            flatten_attribute_into(
                source_layer,
                &attr,
                &dest_spec,
                offset,
                resolve_asset_path_fn,
            );
        } else if let Some(rel) = property.as_relationship() {
            flatten_relationship_into(&rel, &dest_spec, resolve_asset_path_fn);
        }
    }

    // Recursively copy children
    for child in source_spec.name_children() {
        flatten_prim_spec_into(
            source_layer,
            &child,
            destination,
            offset,
            resolve_asset_path_fn,
        );
    }
}

/// Copies prim metadata from source to dest.
fn copy_prim_metadata(
    source: &usd_sdf::prim_spec::PrimSpec,
    dest: &mut usd_sdf::prim_spec::PrimSpec,
) {
    // Copy documentation
    let doc = source.documentation();
    if !doc.is_empty() {
        dest.set_documentation(&doc);
    }

    // Copy comment
    let comment = source.comment();
    if !comment.is_empty() {
        dest.set_comment(&comment);
    }

    // Copy hidden flag
    if source.hidden() {
        dest.set_hidden(true);
    }

    // Copy active flag
    if !source.active() {
        dest.set_active(false);
    }
}

/// Flattens an attribute into the destination prim spec.
fn flatten_attribute_into(
    source_layer: &Arc<Layer>,
    source_attr: &usd_sdf::attribute_spec::AttributeSpec,
    dest_prim: &usd_sdf::prim_spec::PrimSpec,
    offset: LayerOffset,
    resolve_asset_path_fn: Option<&BoxedResolveAssetPathFn>,
) {
    use usd_sdf::attribute_spec::AttributeSpec;

    let source_path = source_attr.path();
    let attr_name = source_path.get_name();
    let type_name = source_attr.type_name();

    // Build destination attribute path
    let dest_attr_path = match dest_prim.path().append_property(attr_name) {
        Some(p) => p,
        None => return,
    };

    // Create the attribute in destination
    let layer = dest_prim.layer();
    let mut dest_attr = AttributeSpec::from_layer_and_path(layer, dest_attr_path);
    dest_attr.set_type_name(&type_name);

    // Copy default value
    let default_val = source_attr.default_value();
    if !default_val.is_empty() {
        let resolved = resolve_asset_value(source_layer, &default_val, resolve_asset_path_fn);
        dest_attr.set_default_value(resolved);
    }

    // Copy time samples with offset applied
    if source_attr.has_time_samples() {
        let samples = source_attr.time_sample_map();
        for (ordered_time, value) in samples {
            let time: f64 = ordered_time.into();
            let offset_time = offset * time;
            let resolved = resolve_asset_value(source_layer, &value, resolve_asset_path_fn);
            dest_attr.set_time_sample(offset_time, resolved);
        }
    }
}

/// Flattens a relationship into the destination prim spec.
fn flatten_relationship_into(
    source_rel: &usd_sdf::relationship_spec::RelationshipSpec,
    dest_prim: &usd_sdf::prim_spec::PrimSpec,
    _resolve_asset_path_fn: Option<&BoxedResolveAssetPathFn>,
) {
    use usd_sdf::relationship_spec::RelationshipSpec;

    let rel_name = source_rel.name();
    let rel_path = dest_prim.path().append_property(&rel_name);
    let rel_path = match rel_path {
        Some(p) => p,
        None => return,
    };

    // Create the relationship in destination
    let layer = dest_prim.layer();
    let mut dest_rel = RelationshipSpec::new(layer, rel_path);

    // Copy targets
    let targets = source_rel.target_path_list();
    dest_rel.set_target_path_list(targets);
}

/// Resolves asset paths in a value using the callback.
///
/// Checks if the value is an SdfAssetPath or contains asset paths,
/// and applies the resolve function to transform the path.
/// Matches C++ `_ResolveAssetPath` in flattenLayerStack.cpp.
fn resolve_asset_value(
    source_layer: &Arc<Layer>,
    value: &usd_sdf::abstract_data::Value,
    resolve_fn: Option<&BoxedResolveAssetPathFn>,
) -> usd_sdf::abstract_data::Value {
    let resolver = match resolve_fn {
        Some(f) => f,
        // No custom resolver: use default resolution
        None => {
            // Try to resolve asset paths with the default resolver
            if let Some(asset_path) = value.get::<usd_sdf::AssetPath>() {
                let raw = asset_path.get_asset_path();
                if !raw.is_empty() {
                    let resolved = flatten_layer_stack_resolve_asset_path(source_layer, &raw);
                    return usd_sdf::abstract_data::Value::new(usd_sdf::AssetPath::new(&resolved));
                }
            }
            // For arrays of asset paths
            if let Some(paths) = value.get::<Vec<usd_sdf::AssetPath>>() {
                let resolved: Vec<usd_sdf::AssetPath> = paths
                    .iter()
                    .map(|ap| {
                        let raw = ap.get_asset_path();
                        if raw.is_empty() {
                            ap.clone()
                        } else {
                            let resolved =
                                flatten_layer_stack_resolve_asset_path(source_layer, &raw);
                            usd_sdf::AssetPath::new(&resolved)
                        }
                    })
                    .collect();
                return usd_sdf::abstract_data::Value::new(resolved);
            }
            return value.clone();
        }
    };

    // Custom resolver provided
    if let Some(asset_path) = value.get::<usd_sdf::AssetPath>() {
        let raw = asset_path.get_asset_path();
        if !raw.is_empty() {
            let resolved = resolver(source_layer, &raw);
            return usd_sdf::abstract_data::Value::new(usd_sdf::AssetPath::new(&resolved));
        }
    }

    // For arrays of asset paths
    if let Some(paths) = value.get::<Vec<usd_sdf::AssetPath>>() {
        let resolved: Vec<usd_sdf::AssetPath> = paths
            .iter()
            .map(|ap| {
                let raw = ap.get_asset_path();
                if raw.is_empty() {
                    ap.clone()
                } else {
                    let resolved = resolver(source_layer, &raw);
                    usd_sdf::AssetPath::new(&resolved)
                }
            })
            .collect();
        return usd_sdf::abstract_data::Value::new(resolved);
    }

    value.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_offset_identity() {
        let offset = LayerOffset::default();
        assert_eq!(offset * 10.0, 10.0);
        assert_eq!(offset * 100.0, 100.0);
    }

    #[test]
    fn test_layer_offset_apply() {
        let offset = LayerOffset::new(10.0, 2.0);
        assert_eq!(offset * 5.0, 20.0); // 5 * 2 + 10 = 20
    }

    #[test]
    fn test_layer_offset_compose() {
        let a = LayerOffset::new(10.0, 2.0);
        let b = LayerOffset::new(5.0, 0.5);
        let composed = a * b;
        // Matches C++ SdfLayerOffset::operator*: applies b first, then a
        // new_scale = a.scale * b.scale = 2.0 * 0.5 = 1.0
        // new_offset = a.scale * b.offset + a.offset = 2.0 * 5.0 + 10.0 = 20.0
        assert_eq!(composed * 0.0, 20.0);
    }

    #[test]
    fn test_resolve_asset_path_absolute() {
        let layer = Layer::create_anonymous(Some("test"));
        let result = flatten_layer_stack_resolve_asset_path(&layer, "/absolute/path.usd");
        assert_eq!(result, "/absolute/path.usd");
    }

    #[test]
    fn test_resolve_asset_path_empty() {
        let layer = Layer::create_anonymous(Some("test"));
        let result = flatten_layer_stack_resolve_asset_path(&layer, "");
        assert_eq!(result, "");
    }
}
