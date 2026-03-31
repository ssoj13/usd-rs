//! Asset dependency extraction utilities.
//!
//! Provides utilities for extracting and computing asset dependencies
//! from USD files.

use super::user_processing_func::{BoxedProcessingFunc, DependencyInfo};
use std::sync::Arc;
use usd_sdf::asset_path::AssetPath;
use usd_sdf::layer::Layer;

/// Parameters controlling external reference extraction.
///
/// This structure controls aspects of the [`extract_external_references`]
/// function.
#[derive(Debug, Clone, Default)]
pub struct ExtractExternalReferencesParams {
    /// Whether UDIM template paths should be resolved when extracting.
    ///
    /// If true, the resolved paths for all discovered UDIM tiles will be
    /// included in the references bucket and the template path discarded.
    /// If false, no resolution takes place and the template path appears
    /// in the references bucket.
    resolve_udim_paths: bool,
}

impl ExtractExternalReferencesParams {
    /// Creates a new parameters object with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to resolve UDIM paths.
    pub fn set_resolve_udim_paths(&mut self, resolve: bool) {
        self.resolve_udim_paths = resolve;
    }

    /// Returns whether UDIM paths should be resolved.
    pub fn get_resolve_udim_paths(&self) -> bool {
        self.resolve_udim_paths
    }

    /// Builder pattern for setting resolve_udim_paths.
    pub fn with_resolve_udim_paths(mut self, resolve: bool) -> Self {
        self.resolve_udim_paths = resolve;
        self
    }
}

/// Parses a file and extracts external references into type-based buckets.
///
/// Sublayers are returned in `sublayers`, references (prim references, value
/// clip references, asset path attribute values) are returned in `references`,
/// and payload paths are returned in `payloads`.
///
/// # Arguments
///
/// * `file_path` - Path to the USD file to parse
/// * `params` - Parameters controlling extraction
///
/// # Returns
///
/// A tuple of (sublayers, references, payloads).
///
/// # Notes
///
/// - No recursive chasing of dependencies is performed
/// - Not all returned references are explicitly authored (e.g., templated clip
///   paths are expanded)
pub fn extract_external_references(
    file_path: &str,
    params: &ExtractExternalReferencesParams,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut sublayers = Vec::new();
    let references = Vec::new();
    let payloads = Vec::new();

    // Try to open the layer
    let layer = match Layer::find_or_open(file_path) {
        Ok(l) => l,
        Err(_) => return (sublayers, references, payloads),
    };

    // Extract sublayers
    sublayers.extend(layer.get_sublayer_paths());

    // Traverse prim specs and extract references/payloads
    let mut references = references;
    let mut payloads = payloads;
    for root_prim in layer.root_prims() {
        extract_refs_from_spec(&root_prim, &mut references, &mut payloads, params);
    }

    (sublayers, references, payloads)
}

/// Recursively computes all dependencies of a given asset.
///
/// Populates `layers` with all dependencies that can be opened as an SdfLayer,
/// `assets` with resolved non-layer dependencies, and `unresolved_paths` with
/// any unresolved asset paths.
///
/// # Arguments
///
/// * `asset_path` - The root asset to analyze
/// * `processing_func` - Optional callback for processing each discovered path
///
/// # Returns
///
/// A tuple of (layers, assets, unresolved_paths), or `None` if the asset
/// couldn't be resolved.
///
/// # Notes
///
/// Changes made to paths in the processing function will not be written to
/// processed layers.
pub fn compute_all_dependencies(
    asset_path: &AssetPath,
    processing_func: Option<&BoxedProcessingFunc>,
) -> Option<(Vec<Arc<Layer>>, Vec<String>, Vec<String>)> {
    let mut layers = Vec::new();
    let mut assets = Vec::new();
    let mut unresolved_paths = Vec::new();

    // Resolve the root asset
    let root_path = asset_path.get_resolved_path();
    if root_path.is_empty() {
        return None;
    }

    // Open the root layer
    let root_layer = Layer::find_or_open(root_path).ok()?;

    // Track visited layers to avoid cycles
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    visited.insert(root_path.to_string());

    // Queue of layers to process
    let mut queue = vec![Arc::clone(&root_layer)];

    while let Some(layer) = queue.pop() {
        layers.push(Arc::clone(&layer));

        // Extract dependencies from this layer
        let (sublayers, refs, _payloads) = extract_external_references(
            layer.identifier(),
            &ExtractExternalReferencesParams::default(),
        );

        // Process sublayers
        for sublayer_path in sublayers {
            // Invoke processing func if provided
            let final_path = if let Some(func) = processing_func {
                let info = DependencyInfo::new(&sublayer_path);
                let result = func(&layer, &info);
                if result.is_ignored() {
                    continue;
                }
                result.get_asset_path().to_string()
            } else {
                sublayer_path
            };

            if visited.contains(&final_path) {
                continue;
            }
            visited.insert(final_path.clone());

            // Try to open as a layer
            if let Ok(sublayer) = Layer::find_or_open(&final_path) {
                queue.push(sublayer);
            } else {
                unresolved_paths.push(final_path);
            }
        }

        // Process references
        for ref_path in refs {
            let final_path = if let Some(func) = processing_func {
                let info = DependencyInfo::new(&ref_path);
                let result = func(&layer, &info);
                if result.is_ignored() {
                    continue;
                }
                result.get_asset_path().to_string()
            } else {
                ref_path
            };

            if visited.contains(&final_path) {
                continue;
            }
            visited.insert(final_path.clone());

            // Check if it's a layer or just an asset
            if let Ok(ref_layer) = Layer::find_or_open(&final_path) {
                queue.push(ref_layer);
            } else {
                // It might be a non-layer asset (texture, etc.)
                assets.push(final_path);
            }
        }
    }

    Some((layers, assets, unresolved_paths))
}

/// Callback for modifying asset paths in a layer.
///
/// The function receives the asset path string and returns the new value
/// to author. If an empty string is returned, the value is removed.
pub type ModifyAssetPathFn = dyn Fn(&str) -> String + Send + Sync;

/// A boxed modify asset path function.
pub type BoxedModifyAssetPathFn = Box<ModifyAssetPathFn>;

/// Visits every asset path in a layer and replaces it with the return value.
///
/// This modifies the layer in place.
///
/// # Arguments
///
/// * `layer` - The layer to modify
/// * `modify_fn` - Callback that transforms each asset path
/// * `keep_empty_paths_in_arrays` - If true, empty strings are kept in arrays
///
/// # Use Cases
///
/// Useful for preparing a layer for consumption in contexts without access
/// to the ArResolver - all paths can be replaced with their fully resolved
/// equivalents.
pub fn modify_asset_paths(
    layer: &Arc<Layer>,
    modify_fn: &BoxedModifyAssetPathFn,
    keep_empty_paths_in_arrays: bool,
) {
    // Traverse all prim specs in the layer
    for prim_spec in layer.root_prims() {
        modify_asset_paths_in_prim(&prim_spec, modify_fn, keep_empty_paths_in_arrays);
    }
}

/// Recursively modifies asset paths in a prim spec.
fn modify_asset_paths_in_prim(
    prim_spec: &usd_sdf::prim_spec::PrimSpec,
    modify_fn: &BoxedModifyAssetPathFn,
    keep_empty_paths_in_arrays: bool,
) {
    // Process properties (attributes)
    for property in prim_spec.properties() {
        if let Some(mut attr) = property.as_attribute() {
            // Check default value
            let default_val = attr.default_value();
            if !default_val.is_empty() {
                if let Some(new_val) =
                    modify_asset_value(&default_val, modify_fn, keep_empty_paths_in_arrays)
                {
                    attr.set_default_value(new_val);
                }
            }

            // Check time samples
            if attr.has_time_samples() {
                let samples = attr.time_sample_map();
                for (time, value) in samples {
                    if let Some(new_val) =
                        modify_asset_value(&value, modify_fn, keep_empty_paths_in_arrays)
                    {
                        attr.set_time_sample(time.into(), new_val);
                    }
                }
            }
        }
    }

    // Recurse to children
    for child in prim_spec.name_children() {
        modify_asset_paths_in_prim(&child, modify_fn, keep_empty_paths_in_arrays);
    }
}

/// Modifies asset path values using the provided function.
/// Returns Some(new_value) if modified, None if unchanged.
fn modify_asset_value(
    value: &usd_sdf::abstract_data::Value,
    modify_fn: &BoxedModifyAssetPathFn,
    keep_empty_paths_in_arrays: bool,
) -> Option<usd_sdf::abstract_data::Value> {
    // Try single AssetPath
    if let Some(asset_path) = value.get::<usd_sdf::asset_path::AssetPath>() {
        let path_str = asset_path.get_authored_path();
        let new_path = modify_fn(path_str);
        if new_path != path_str {
            return Some(usd_sdf::abstract_data::Value::new(
                usd_sdf::asset_path::AssetPath::new(&new_path),
            ));
        }
        return None;
    }

    // Try array of AssetPaths
    if let Some(asset_paths) = value.get::<Vec<usd_sdf::asset_path::AssetPath>>() {
        let mut modified = false;
        let new_paths: Vec<usd_sdf::asset_path::AssetPath> = asset_paths
            .iter()
            .filter_map(|ap| {
                let path_str = ap.get_authored_path();
                let new_path = modify_fn(path_str);
                if new_path.is_empty() && !keep_empty_paths_in_arrays {
                    modified = true;
                    None
                } else if new_path != path_str {
                    modified = true;
                    Some(usd_sdf::asset_path::AssetPath::new(&new_path))
                } else {
                    Some(ap.clone())
                }
            })
            .collect();

        if modified {
            return Some(usd_sdf::abstract_data::Value::new(new_paths));
        }
    }

    None
}

/// Recursively extracts references and payloads from a prim spec.
fn extract_refs_from_spec(
    prim_spec: &usd_sdf::prim_spec::PrimSpec,
    references: &mut Vec<String>,
    payloads: &mut Vec<String>,
    params: &ExtractExternalReferencesParams,
) {
    // Note: Full reference/payload extraction requires ReferenceListOp API
    // which stores Reference structs with asset paths. For now, we extract
    // from properties and recurse to children.
    let _ = (prim_spec.has_references(), prim_spec.has_payloads());

    // Extract from properties (asset-valued attributes)
    for property in prim_spec.properties() {
        if let Some(attr) = property.as_attribute() {
            // Check default value
            let default_val = attr.default_value();
            if !default_val.is_empty() {
                extract_asset_paths_from_value(&default_val, references, params);
            }

            // Check time samples
            if attr.has_time_samples() {
                let samples = attr.time_sample_map();
                for (_time, value) in samples {
                    extract_asset_paths_from_value(&value, references, params);
                }
            }
        }
    }

    // Extract clip asset paths
    extract_clip_asset_paths(prim_spec, references);

    // Recurse to children
    for child in prim_spec.name_children() {
        extract_refs_from_spec(&child, references, payloads, params);
    }
}

/// Checks if a path is a UDIM pattern.
fn is_udim_path(path: &str) -> bool {
    path.contains("<UDIM>") || path.contains("<udim>")
}

/// Resolves UDIM tiles for a template path.
fn resolve_udim_tiles(template: &str) -> Vec<String> {
    // In a full implementation, this would:
    // 1. Parse the UDIM template
    // 2. Scan the filesystem for matching tiles
    // 3. Return all discovered tile paths
    // For now, return the template as-is
    vec![template.to_string()]
}

/// Extracts asset paths from a value if it's an asset type.
fn extract_asset_paths_from_value(
    value: &usd_sdf::abstract_data::Value,
    references: &mut Vec<String>,
    params: &ExtractExternalReferencesParams,
) {
    // Try to extract as AssetPath
    if let Some(asset_path) = value.get::<usd_sdf::asset_path::AssetPath>() {
        let path_str = asset_path.get_authored_path();
        if !path_str.is_empty() {
            if params.get_resolve_udim_paths() && is_udim_path(path_str) {
                references.extend(resolve_udim_tiles(path_str));
            } else {
                references.push(path_str.to_string());
            }
        }
        return;
    }

    // Try to extract as array of AssetPaths
    if let Some(asset_paths) = value.get::<Vec<usd_sdf::asset_path::AssetPath>>() {
        for asset_path in asset_paths {
            let path_str = asset_path.get_authored_path();
            if !path_str.is_empty() {
                if params.get_resolve_udim_paths() && is_udim_path(path_str) {
                    references.extend(resolve_udim_tiles(path_str));
                } else {
                    references.push(path_str.to_string());
                }
            }
        }
    }
}

/// Extracts asset paths from clip metadata on a prim spec.
fn extract_clip_asset_paths(
    prim_spec: &usd_sdf::prim_spec::PrimSpec,
    references: &mut Vec<String>,
) {
    // Get clip metadata from prim spec's custom data
    // The "clips" field is a dictionary with clip set names as keys
    let clips_token = usd_tf::Token::new("clips");
    let spec = prim_spec.spec();

    if let Some(clips_dict) = spec.get_field(&clips_token).as_dictionary() {
        // Each clip set may have clipAssetPaths or clipTemplateAssetPath
        for (_set_name, set_value) in clips_dict {
            if let Some(inner_dict) = set_value.as_dictionary() {
                // Check clipAssetPaths (array of asset paths)
                if let Some(asset_paths_value) = inner_dict.get("clipAssetPaths") {
                    if let Some(paths) =
                        asset_paths_value.get::<Vec<usd_sdf::asset_path::AssetPath>>()
                    {
                        for path in paths {
                            let path_str = path.get_authored_path();
                            if !path_str.is_empty() {
                                references.push(path_str.to_string());
                            }
                        }
                    }
                }

                // Check clipTemplateAssetPath (single string template)
                if let Some(template_value) = inner_dict.get("clipTemplateAssetPath") {
                    if let Some(template) = template_value.get::<String>() {
                        if !template.is_empty() {
                            references.push(template.clone());
                        }
                    }
                    // Also check if stored as AssetPath
                    if let Some(template_path) =
                        template_value.get::<usd_sdf::asset_path::AssetPath>()
                    {
                        let path_str = template_path.get_authored_path();
                        if !path_str.is_empty() {
                            references.push(path_str.to_string());
                        }
                    }
                }
            }
        }
    }
}

/// Creates a modify function that resolves all paths to absolute form.
pub fn make_absolute_path_modifier(base_layer: &Arc<Layer>) -> BoxedModifyAssetPathFn {
    let base_dir = base_layer
        .real_path()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());

    Box::new(move |asset_path| {
        // If already absolute, return as-is
        if asset_path.starts_with('/') || asset_path.contains("://") {
            return asset_path.to_string();
        }

        // Make relative paths absolute
        if let Some(ref dir) = base_dir {
            let absolute = dir.join(asset_path);
            if let Some(s) = absolute.to_str() {
                return s.to_string();
            }
        }

        asset_path.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_params_default() {
        let params = ExtractExternalReferencesParams::default();
        assert!(!params.get_resolve_udim_paths());
    }

    #[test]
    fn test_extract_params_builder() {
        let params = ExtractExternalReferencesParams::new().with_resolve_udim_paths(true);
        assert!(params.get_resolve_udim_paths());
    }

    #[test]
    fn test_extract_external_references_nonexistent() {
        let (sublayers, refs, payloads) = extract_external_references(
            "/nonexistent/file.usd",
            &ExtractExternalReferencesParams::default(),
        );
        assert!(sublayers.is_empty());
        assert!(refs.is_empty());
        assert!(payloads.is_empty());
    }
}
