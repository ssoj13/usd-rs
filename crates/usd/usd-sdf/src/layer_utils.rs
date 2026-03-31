//! Layer utility functions.
//!
//! Port of pxr/usd/sdf/layerUtils.h
//!
//! Provides utilities for working with layer asset paths.

use crate::Layer;
use crate::asset_path_resolver::is_package_or_packaged_layer;
use crate::file_format::find_format_by_extension;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use usd_ar::{ResolvedPath, get_resolver};

// ============================================================================
// Private helpers (matching C++ anonymous namespace in layerUtils.cpp)
// ============================================================================

/// Anchors relativePath to the directory of anchorLayerPath.
///
/// Equivalent to C++ `_AnchorRelativePath` (layerUtils.cpp:31-36):
///   anchorPath = TfGetPathName(anchorLayerPath)
///   return anchorPath.empty() ? relativePath : anchorPath + "/" + relativePath
fn anchor_relative_path(anchor_layer_path: &str, relative_path: &str) -> String {
    // TfGetPathName: directory part (everything up to and including last '/')
    let anchor_dir = if let Some(pos) = anchor_layer_path.rfind('/') {
        &anchor_layer_path[..=pos]
    } else {
        ""
    };
    if anchor_dir.is_empty() {
        relative_path.to_string()
    } else {
        format!("{}{}", anchor_dir, relative_path)
    }
}

/// Splits a package-relative path at the innermost '[' bracket.
///
/// Equivalent to C++ `ArSplitPackageRelativePathInner`.
/// E.g. "foo.usdz[bar/baz.usda]" -> ("foo.usdz", "bar/baz.usda")
/// E.g. "foo.usdz" -> ("foo.usdz", "")
fn ar_split_package_relative_path_inner(path: &str) -> (String, String) {
    // Find the last '[' — that's the innermost package boundary.
    if let Some(open) = path.rfind('[') {
        let package = path[..open].to_string();
        let inner = if path.ends_with(']') {
            path[open + 1..path.len() - 1].to_string()
        } else {
            path[open + 1..].to_string()
        };
        (package, inner)
    } else {
        (path.to_string(), String::new())
    }
}

/// Joins a package path and packaged path into a package-relative path.
///
/// Equivalent to C++ `ArJoinPackageRelativePath`.
/// E.g. ("foo.usdz", "bar.usda") -> "foo.usdz[bar.usda]"
/// E.g. ("foo.usdz", "") -> "foo.usdz"
pub fn ar_join_package_relative_path(package: &str, packaged: &str) -> String {
    if packaged.is_empty() {
        package.to_string()
    } else {
        format!("{}[{}]", package, packaged)
    }
}

/// Normalizes a path (collapses "..", ".", duplicate slashes).
///
/// Equivalent to C++ `TfNormPath` (simplified — forward slashes only).
fn tf_norm_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let normalized = parts.join("/");
    // Preserve leading './' if original starts with it
    if path.starts_with("./") {
        format!("./{}", normalized)
    } else {
        normalized
    }
}

/// Expands a (package_path, packaged_path) pair until the packaged path is
/// a non-package layer (the root layer of the innermost package).
///
/// Equivalent to C++ `_ExpandPackagePath` (layerUtils.cpp:42-60).
fn expand_package_path(package: String, packaged: String) -> (String, String) {
    let mut result = (package, packaged);
    loop {
        if result.1.is_empty() {
            break;
        }
        // Check if the packaged path itself refers to another package
        let ext = StdPath::new(&result.1)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let packaged_format = find_format_by_extension(ext, None);
        let is_pkg = packaged_format.as_ref().map_or(false, |f| f.is_package());
        if !is_pkg {
            break;
        }
        // Join and get the root of the nested package
        let joined = ar_join_package_relative_path(&result.0, &result.1);
        let root = packaged_format
            .unwrap()
            .get_package_root_layer_path(&ResolvedPath::new(&joined));
        result = (joined, root);
    }
    result
}

/// Computes the asset path relative to the anchor layer.
///
/// Full implementation matching C++ `SdfComputeAssetPathRelativeToLayer`
/// (layerUtils.cpp:64-194), including the USDZ/package-relative path
/// resolution logic (lines 117-183).
///
/// Order:
/// 1. Strip file format args
/// 2. If anchor is package/packaged and strippedPath is relative:
///    a. Build anchorPackagePath (repo path or real path)
///    b. Split anchor into (package, packaged) pair
///    c. Anchor stripped path relative to packaged inner path
///    d. If anchored path starts with '.' (non-search-relative): return immediately
///    e. Try resolver.Resolve — if found: return
///    f. Try anchoring to package root layer — if found: return
///    g. Fall through to normal resolution
/// 3. Anonymous asset path: return as-is
/// 4. Anonymous anchor: create identifier without anchor
/// 5. Normal: create identifier anchored to anchor resolved path
pub fn compute_asset_path_relative_to_layer(anchor: &Arc<Layer>, asset_path: &str) -> String {
    if asset_path.is_empty() {
        return String::new();
    }

    // Step 1: strip file format arguments (C++: Sdf_SplitIdentifier)
    let (stripped_path, layer_args) = split_identifier(asset_path);
    if stripped_path.is_empty() {
        return String::new();
    }
    let layer_args_str = layer_args.unwrap_or("");

    // Step 2: USDZ/package-relative path handling
    // C++ layerUtils.cpp:117-183
    let stripped_is_relative =
        !stripped_path.starts_with('/') && !StdPath::new(stripped_path).is_absolute();

    if is_package_or_packaged_layer(anchor) && stripped_is_relative {
        // Use repository path if available, otherwise real path
        // C++ layerUtils.cpp:121-122
        let anchor_package_path = anchor
            .get_repository_path()
            .filter(|s| !s.is_empty())
            .or_else(|| anchor.get_resolved_path())
            .unwrap_or_default();

        // Split anchor into (package, packaged) pair.
        // If anchor IS a package format, start from its root layer.
        // Otherwise split by innermost bracket.
        // C++ layerUtils.cpp:128-138
        let (pkg_path, packaged_path) = {
            let is_anchor_package = anchor.get_file_format().map_or(false, |f| f.is_package());
            if is_anchor_package {
                // anchor is the package itself: get its root layer
                let root = anchor
                    .get_file_format()
                    .unwrap()
                    .get_package_root_layer_path(&ResolvedPath::new(
                        anchor
                            .real_path()
                            .map(|p| p.to_string_lossy())
                            .unwrap_or_default()
                            .as_ref(),
                    ));
                expand_package_path(anchor_package_path.clone(), root)
            } else {
                // anchor is a packaged layer: split at the innermost '['
                ar_split_package_relative_path_inner(&anchor_package_path)
            }
        };

        // Normalize and anchor the asset path to the packaged inner path
        // C++ layerUtils.cpp:140-144
        let norm_asset_path = tf_norm_path(stripped_path);
        let anchored_packaged = anchor_relative_path(&packaged_path, &norm_asset_path);
        let final_layer_path = ar_join_package_relative_path(&pkg_path, &anchored_packaged);

        // If not a search-relative path (starts with '.'), we're done.
        // C++ layerUtils.cpp:148-151
        let is_search_relative = !stripped_path.starts_with('.');
        if !is_search_relative {
            return create_identifier_with_args(&final_layer_path, layer_args_str);
        }

        // Try resolving the anchored path.
        // C++ layerUtils.cpp:155-157
        {
            let resolver = get_resolver().read().expect("rwlock poisoned");
            if !resolver.resolve(&final_layer_path).is_empty() {
                return create_identifier_with_args(&final_layer_path, layer_args_str);
            }
        }

        // Try anchoring to the owning package's root layer.
        // C++ layerUtils.cpp:162-178
        let final_layer_path2 = {
            let pkg_ext = StdPath::new(&pkg_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let package_format = find_format_by_extension(pkg_ext, None);
            if package_format.as_ref().map_or(false, |f| f.is_package()) {
                let root = package_format
                    .unwrap()
                    .get_package_root_layer_path(&ResolvedPath::new(&pkg_path));
                let (pkg2, packaged2) = expand_package_path(pkg_path.clone(), root);
                let anchored2 = anchor_relative_path(&packaged2, &norm_asset_path);
                ar_join_package_relative_path(&pkg2, &anchored2)
            } else {
                // No package format: use norm_asset_path directly relative to pkg
                ar_join_package_relative_path(&pkg_path, &norm_asset_path)
            }
        };

        {
            let resolver = get_resolver().read().expect("rwlock poisoned");
            if !resolver.resolve(&final_layer_path2).is_empty() {
                return create_identifier_with_args(&final_layer_path2, layer_args_str);
            }
        }

        // Fall through to normal path resolution (C++ layerUtils.cpp:183)
    }

    // Step 3: Anonymous asset path passes through unchanged
    // C++ layerUtils.cpp:185-187
    if Layer::is_anonymous_layer_identifier(stripped_path) {
        return create_identifier_with_args(stripped_path, layer_args_str);
    }

    let resolver = get_resolver().read().expect("rwlock poisoned");

    // Step 4: Anonymous anchor — no anchor path
    // C++ layerUtils.cpp:189-191
    if anchor.is_anonymous() {
        let id = resolver.create_identifier(stripped_path, None);
        return create_identifier_with_args(&id, layer_args_str);
    }

    // Step 5: Normal anchor
    // C++ layerUtils.cpp:193
    let anchor_resolved = anchor.get_resolved_path().map(|p| ResolvedPath::new(p));
    let id = resolver.create_identifier(stripped_path, anchor_resolved.as_ref());
    create_identifier_with_args(&id, layer_args_str)
}

/// Resolves the asset path relative to the anchor layer.
///
/// Combines `compute_asset_path_relative_to_layer` with `ArResolver::resolve()`
/// to get the final physical path. Matches C++ `SdfResolveAssetPathRelativeToLayer`.
pub fn resolve_asset_path_relative_to_layer(anchor: &Arc<Layer>, asset_path: &str) -> String {
    if asset_path.is_empty() {
        return asset_path.to_string();
    }

    // Anonymous layer identifiers don't need resolution
    if Layer::is_anonymous_layer_identifier(asset_path) {
        return asset_path.to_string();
    }

    let computed = compute_asset_path_relative_to_layer(anchor, asset_path);

    // Resolve via ArResolver (C++ uses ArGetResolver().Resolve())
    let resolver = get_resolver().read().expect("rwlock poisoned");
    let resolved = resolver.resolve(&computed);
    if !resolved.is_empty() {
        return resolved.into_string();
    }

    computed
}

/// Splits a layer identifier into its path and arguments.
///
/// Layer identifiers can have arguments like: `path.usd:SDF_FORMAT_ARGS:arg=value`
pub fn split_identifier(identifier: &str) -> (&str, Option<&str>) {
    if let Some(idx) = identifier.find(":SDF_FORMAT_ARGS:") {
        (&identifier[..idx], Some(&identifier[idx + 17..]))
    } else {
        (identifier, None)
    }
}

/// Creates a layer identifier with format arguments.
pub fn create_identifier_with_args(path: &str, args: &str) -> String {
    if args.is_empty() {
        path.to_string()
    } else {
        format!("{}:SDF_FORMAT_ARGS:{}", path, args)
    }
}

/// Returns the file extension from a layer identifier.
pub fn get_extension(identifier: &str) -> &str {
    let (path, _) = split_identifier(identifier);
    StdPath::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
}

/// Returns the directory containing the layer.
pub fn get_layer_directory(layer: &Arc<Layer>) -> Option<PathBuf> {
    layer
        .real_path()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// Checks if two layer identifiers refer to the same layer.
pub fn identifiers_equal(id1: &str, id2: &str) -> bool {
    let (path1, args1) = split_identifier(id1);
    let (path2, args2) = split_identifier(id2);

    // Compare paths (case-insensitive on Windows)
    #[cfg(windows)]
    let paths_equal = path1.eq_ignore_ascii_case(path2);
    #[cfg(not(windows))]
    let paths_equal = path1 == path2;

    paths_equal && args1 == args2
}

/// Makes a path relative to a base directory.
pub fn make_relative_path(path: &str, base_dir: &StdPath) -> String {
    let path = StdPath::new(path);

    if let Ok(relative) = path.strip_prefix(base_dir) {
        relative.to_string_lossy().to_string()
    } else {
        path.to_string_lossy().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_identifier() {
        let (path, args) = split_identifier("test.usd");
        assert_eq!(path, "test.usd");
        assert!(args.is_none());

        let (path, args) = split_identifier("test.usd:SDF_FORMAT_ARGS:foo=bar");
        assert_eq!(path, "test.usd");
        assert_eq!(args, Some("foo=bar"));
    }

    #[test]
    fn test_create_identifier_with_args() {
        assert_eq!(create_identifier_with_args("test.usd", ""), "test.usd");
        assert_eq!(
            create_identifier_with_args("test.usd", "arg=value"),
            "test.usd:SDF_FORMAT_ARGS:arg=value"
        );
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("test.usd"), "usd");
        assert_eq!(get_extension("test.usda"), "usda");
        assert_eq!(get_extension("path/to/test.usdc"), "usdc");
        assert_eq!(get_extension("test.usd:SDF_FORMAT_ARGS:a=b"), "usd");
    }

    #[test]
    fn test_identifiers_equal() {
        assert!(identifiers_equal("test.usd", "test.usd"));
        assert!(!identifiers_equal("test.usd", "other.usd"));
        assert!(identifiers_equal(
            "test.usd:SDF_FORMAT_ARGS:a=b",
            "test.usd:SDF_FORMAT_ARGS:a=b"
        ));
    }
}
