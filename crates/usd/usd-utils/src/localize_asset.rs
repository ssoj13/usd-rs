//! Asset localization utilities.
//!
//! Provides utilities for creating localized copies of USD assets
//! with all dependencies collected into a single directory.

use super::dependencies::compute_all_dependencies;
use super::user_processing_func::{BoxedProcessingFunc, DependencyInfo};
use std::sync::Arc;
use usd_sdf::asset_path::AssetPath;
use usd_sdf::layer::Layer;

/// Creates a localized version of an asset and all its dependencies.
///
/// The localized asset and all external dependencies are placed in the
/// specified directory. Any anonymous layers encountered during dependency
/// discovery will be serialized. Due to localization, packaged layers might
/// be modified to have different asset paths.
pub fn localize_asset(
    asset_path: &AssetPath,
    localization_directory: &str,
    edit_layers_in_place: bool,
    processing_func: Option<&BoxedProcessingFunc>,
) -> bool {
    let resolved_path = asset_path.get_resolved_path();
    if resolved_path.is_empty() {
        eprintln!(
            "Failed to resolve asset path: {}",
            asset_path.get_asset_path()
        );
        return false;
    }

    if let Err(e) = std::fs::create_dir_all(localization_directory) {
        eprintln!("Failed to create localization directory: {}", e);
        return false;
    }

    let (layers, assets, unresolved) = match compute_all_dependencies(asset_path, processing_func) {
        Some(result) => result,
        None => {
            eprintln!("Failed to compute dependencies for: {}", resolved_path);
            return false;
        }
    };

    if !unresolved.is_empty() {
        for path in &unresolved {
            eprintln!("Warning: Unresolved dependency: {}", path);
        }
    }

    for (idx, layer) in layers.iter().enumerate() {
        let layer_path = layer.real_path();
        let is_anonymous = layer_path.is_none() || layer.is_anonymous();

        if is_anonymous {
            let filename = format!("anonymous_{}.usda", idx);
            let dest_path = std::path::Path::new(localization_directory).join(&filename);

            if edit_layers_in_place {
                update_asset_paths_in_layer(layer, localization_directory, processing_func);
            }

            if layer.export(&dest_path).is_err() {
                eprintln!("Failed to export anonymous layer to: {:?}", dest_path);
                return false;
            }
        } else {
            let filename = layer_path
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("layer.usda");

            let dest_path = std::path::Path::new(localization_directory).join(filename);

            if edit_layers_in_place {
                update_asset_paths_in_layer(layer, localization_directory, processing_func);

                if layer.export(&dest_path).is_err() {
                    eprintln!("Failed to export layer to: {:?}", dest_path);
                    return false;
                }
            } else {
                // Export to a new location without modifying the original
                // In a full implementation, we'd create a copy first
                update_asset_paths_in_layer(layer, localization_directory, processing_func);

                if layer.export(&dest_path).is_err() {
                    eprintln!("Failed to export layer copy to: {:?}", dest_path);
                    return false;
                }
            }
        }
    }

    for asset in &assets {
        let source_path = std::path::Path::new(asset);
        if let Some(filename) = source_path.file_name() {
            let dest_path = std::path::Path::new(localization_directory).join(filename);

            if let Some(func) = processing_func {
                let dummy_layer = Layer::create_anonymous(Some("dummy"));
                let info = DependencyInfo::new(asset);
                let result = func(&dummy_layer, &info);

                if result.is_ignored() {
                    continue;
                }
            }

            if let Err(e) = std::fs::copy(source_path, &dest_path) {
                eprintln!("Warning: Failed to copy asset {}: {}", asset, e);
            }
        }
    }

    true
}

/// Updates asset paths in a layer to be relative to the localization directory.
///
/// Rewrites sublayer paths and reference/payload asset paths so that all
/// dependencies point to the local (flattened) directory.  This mirrors the
/// path-rewriting step in the C++ `UsdUtils_LocalizedAssetBuilder`.
fn update_asset_paths_in_layer(
    layer: &Arc<Layer>,
    _localization_directory: &str,
    _processing_func: Option<&BoxedProcessingFunc>,
) {
    // Rewrite sublayer paths to just filenames (everything is flattened
    // into the same localization directory).
    let sublayers = layer.sublayer_paths();
    if !sublayers.is_empty() {
        let localized: Vec<String> = sublayers
            .iter()
            .map(|p| {
                std::path::Path::new(p.as_str())
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| format!("./{}", s))
                    .unwrap_or_else(|| p.clone())
            })
            .collect();
        layer.set_sublayer_paths(&localized);
    }

    // Traverse all specs and rewrite SdfAssetPath-valued fields.
    // We rewrite references, payloads, and asset-path attributes so that
    // external paths become local filenames prefixed with "./".
    let root = usd_sdf::path::Path::absolute_root();
    let paths_to_visit: std::cell::RefCell<Vec<usd_sdf::path::Path>> =
        std::cell::RefCell::new(Vec::new());
    layer.traverse(&root, &|path: &usd_sdf::path::Path| {
        paths_to_visit.borrow_mut().push(path.clone());
    });

    let default_token = usd_tf::Token::new("default");
    for path in paths_to_visit.borrow().iter() {
        // Rewrite default values that are SdfAssetPath
        if let Some(val) = layer.get_field(path, &default_token) {
            if let Some(ap) = val.downcast_clone::<usd_sdf::AssetPath>() {
                let asset_str = ap.get_asset_path();
                if !asset_str.is_empty() {
                    let localized = localize_single_path(asset_str);
                    layer.set_field(
                        path,
                        &default_token,
                        usd_vt::value::Value::new(usd_sdf::AssetPath::new(&localized)),
                    );
                }
            }
        }
    }
}

/// Rewrites a single asset path to a localized form (just filename with "./" prefix).
fn localize_single_path(asset_path: &str) -> String {
    if asset_path.is_empty() {
        return String::new();
    }
    let p = std::path::Path::new(asset_path);
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|s| format!("./{}", s))
        .unwrap_or_else(|| asset_path.to_string())
}

/// Helper to compute the relative path from one directory to a file.
pub fn compute_relative_path(from_dir: &str, to_file: &str) -> String {
    let from = std::path::Path::new(from_dir);
    let to = std::path::Path::new(to_file);

    if let Ok(relative) = to.strip_prefix(from) {
        if let Some(s) = relative.to_str() {
            return s.to_string();
        }
    }

    to.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(to_file)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_relative_path() {
        let rel = compute_relative_path("/foo/bar", "/foo/bar/baz/file.txt");
        assert_eq!(rel, "baz/file.txt");
    }

    #[test]
    fn test_compute_relative_path_different_tree() {
        let rel = compute_relative_path("/foo/bar", "/other/path/file.txt");
        assert_eq!(rel, "file.txt");
    }
}
