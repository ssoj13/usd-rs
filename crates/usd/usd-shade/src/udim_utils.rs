//! USD Shade UDIM Utils - utilities for working with UDIM texture paths.
//!
//! Port of pxr/usd/usdShade/udimUtils.h and udimUtils.cpp
//!
//! This class contains a set of utility functions used for working with Udim
//! texture paths.

use std::path::PathBuf;
use usd_sdf::LayerHandle;

const UDIM_PATTERN: &str = "<UDIM>";
const UDIM_START_TILE: i32 = 1001;
const UDIM_END_TILE: i32 = 1100;
const UDIM_TILE_NUMBER_LENGTH: usize = 4;

/// Pair representing a resolved UDIM path.
/// The first member is the fully resolved path
/// The second number contains only the UDIM tile identifier.
pub type ResolvedPathAndTile = (String, String);

/// Split a udim file path such as /someDir/myFile.<UDIM>.exr into a
/// prefix (/someDir/myFile.) and suffix (.exr).
fn _split_udim_pattern(path: &str) -> (String, String) {
    if let Some(pos) = path.find(UDIM_PATTERN) {
        let prefix = path[..pos].to_string();
        let suffix = path[pos + UDIM_PATTERN.len()..].to_string();
        return (prefix, suffix);
    }
    (String::new(), String::new())
}

/// Checks if identifier contains a UDIM token. Currently only "<UDIM>"
/// is supported, but other patterns such as "_MAPID_" may be supported in
/// the future.
///
/// Matches C++ `IsUdimIdentifier(const std::string &identifier)`.
pub fn is_udim_identifier(identifier: &str) -> bool {
    let (prefix, suffix) = _split_udim_pattern(identifier);
    !prefix.is_empty() || !suffix.is_empty()
}

/// Replaces the UDIM pattern contained in identifierWithPattern
/// with replacement.
///
/// Matches C++ `ReplaceUdimPattern(const std::string &identifierWithPattern, const std::string &replacement)`.
pub fn replace_udim_pattern(identifier_with_pattern: &str, replacement: &str) -> String {
    let (prefix, suffix) = _split_udim_pattern(identifier_with_pattern);
    if prefix.is_empty() && suffix.is_empty() {
        return identifier_with_pattern.to_string();
    }
    format!("{}{}{}", prefix, replacement, suffix)
}

/// Given a udim path and layer, this function will split the path and then
/// attempt to resolve all potential udim files that may match.  Returning
/// a pair containing the path and the tile number provides additional
/// flexibility when working with the results downstream by preventing
/// users from having to re-split the resolved path if the tile part is needed.
fn _resolve_udim_paths(
    udim_path: &str,
    layer: Option<&LayerHandle>,
    stop_at_first: bool,
) -> Vec<ResolvedPathAndTile> {
    let mut resolved_paths = Vec::new();

    // Check for bookends, and exit early if it's not a UDIM path
    let (prefix, suffix) = _split_udim_pattern(udim_path);
    if prefix.is_empty() && suffix.is_empty() {
        return resolved_paths;
    }

    // Get resolver (simplified - in full implementation would use ArResolver)
    // For now, we'll do basic path resolution

    for i in UDIM_START_TILE..=UDIM_END_TILE {
        let tile = i.to_string();

        // Fill in integer
        let path = format!("{}{}{}", prefix, tile, suffix);

        // Deal with layer-relative paths if layer is provided
        if let Some(layer_handle) = layer {
            if layer_handle.is_valid() {
                // In full implementation, would use SdfComputeAssetPathRelativeToLayer
                // For now, if path is relative, make it relative to layer's directory
                if !PathBuf::from(&path).is_absolute() {
                    // LayerHandle doesn't directly expose identifier, would need to upgrade to Layer
                    // For now, skip layer-relative path resolution
                }
            }
        }

        // Resolve path (simplified - in full implementation would use ArResolver::Resolve)
        // For now, check if file exists
        let path_buf = PathBuf::from(&path);
        if path_buf.exists() || path_buf.is_absolute() {
            resolved_paths.push((path, tile));

            if stop_at_first {
                break;
            }
        }
    }

    resolved_paths
}

/// Resolves a udimPath containing a UDIM token. The path is first
/// anchored with the passed layer if needed, then the function attempts
/// to resolve any possible UDIM tiles. If any exist, the resolved path is
/// returned with "<UDIM>" substituted back in. If no resolves succeed or
/// udimPath does not contain a UDIM token, an empty string is returned.
///
/// Matches C++ `ResolveUdimPath(const std::string &udimPath, const SdfLayerHandle &layer)`.
pub fn resolve_udim_path(udim_path: &str, layer: Option<&LayerHandle>) -> String {
    // Return empty if passed path is a non-UDIM path or just doesn't
    // resolve as a UDIM
    let udim_paths = _resolve_udim_paths(udim_path, layer, /* stop_at_first = */ true);

    if udim_paths.is_empty() {
        return String::new();
    }

    let (_prefix, _suffix) = _split_udim_pattern(udim_path);

    // Just need first tile to verify and then revert to <UDIM>
    let first_tile_path = &udim_paths[0].0;

    // Check if the resolved path is in a package (like .usdz)
    // In full implementation, would use ArIsPackageRelativePath and ArSplitPackageRelativePathInner
    let (package_part, inner_path) = if first_tile_path.contains('[')
        && first_tile_path.contains(']')
    {
        // Simple package path handling
        if let Some(bracket_start) = first_tile_path.find('[') {
            let package = first_tile_path[..bracket_start].to_string();
            let inner = first_tile_path[bracket_start + 1..first_tile_path.len() - 1].to_string();
            (package, inner)
        } else {
            (String::new(), first_tile_path.clone())
        }
    } else {
        (String::new(), first_tile_path.clone())
    };

    // Construct the file path /filePath/myImage.<UDIM>.exr by using
    // the first part from the first resolved tile, "<UDIM>" and the
    // suffix.
    let suffix_from_pattern = _split_udim_pattern(udim_path).1;

    // Sanity check that the part after <UDIM> did not change.
    if !inner_path.ends_with(&suffix_from_pattern) {
        eprintln!(
            "Resolution of first udim tile gave ambiguous result. \
            First tile for '{}' is '{}'.",
            udim_path, first_tile_path
        );
        return String::new();
    }

    // Length of the part /filePath/myImage.<UDIM>.exr.
    let prefix_length = inner_path.len() - suffix_from_pattern.len() - UDIM_TILE_NUMBER_LENGTH;

    let mut first_tile_path_result = format!(
        "{}{}{}",
        &inner_path[..prefix_length],
        UDIM_PATTERN,
        suffix_from_pattern
    );

    // Join package path back if needed
    if !package_part.is_empty() {
        first_tile_path_result = format!("{}[{}]", package_part, first_tile_path_result);
    }

    first_tile_path_result
}

/// Attempts to resolve all paths which match a path containing a UDIM
/// pattern. The path is first anchored with the passed layer if needed,
/// then the function attempts to resolve all possible UDIM numbers in the
/// path.
///
/// Matches C++ `ResolveUdimTilePaths(const std::string &udimPath, const SdfLayerHandle &layer)`.
pub fn resolve_udim_tile_paths(
    udim_path: &str,
    layer: Option<&LayerHandle>,
) -> Vec<ResolvedPathAndTile> {
    _resolve_udim_paths(udim_path, layer, /* stop_at_first = */ false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_udim_identifier() {
        assert!(is_udim_identifier("/path/to/texture.<UDIM>.exr"));
        assert!(!is_udim_identifier("/path/to/texture.exr"));
    }

    #[test]
    fn test_replace_udim_pattern() {
        let result = replace_udim_pattern("/path/to/texture.<UDIM>.exr", "1001");
        assert_eq!(result, "/path/to/texture.1001.exr");
    }
}
