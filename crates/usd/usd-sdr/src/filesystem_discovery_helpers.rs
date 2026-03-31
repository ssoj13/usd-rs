//! Filesystem Discovery Helpers - Utilities for discovering shader nodes on the filesystem.
//!
//! Port of pxr/usd/sdr/filesystemDiscoveryHelpers.h
//!
//! This module provides utilities that filesystem discovery plugins use to find
//! shader definitions. If a custom filesystem discovery plugin is needed, these
//! can be used to fill in a large chunk of the functionality.

use std::collections::HashSet;
use std::path::Path;

use super::declare::{SdrStringSet, SdrStringVec, SdrVersion};
use super::discovery_result::{SdrShaderNodeDiscoveryResult, SdrShaderNodeDiscoveryResultVec};
use usd_tf::Token;

/// Type of a function that can be used to parse a discovery result's identifier
/// into its family, name, and version.
pub type SdrParseIdentifierFn =
    Box<dyn Fn(&Token, &mut Token, &mut Token, &mut SdrVersion) -> bool + Send + Sync>;

/// Struct for holding a URI and its resolved URI for a file discovered
/// by discover_files.
#[derive(Debug, Clone)]
pub struct SdrDiscoveryUri {
    /// The original URI.
    pub uri: String,
    /// The resolved URI.
    pub resolved_uri: String,
}

/// A vector of URI/resolved URI structs.
pub type SdrDiscoveryUriVec = Vec<SdrDiscoveryUri>;

/// Given a shader's identifier token, computes the corresponding
/// SdrShaderNode's family name, implementation name and shader version.
///
/// * `family` is the prefix of `identifier` up to and not including the first underscore.
/// * `version` is the suffix of `identifier` comprised of one or two integers
///   representing the major and minor version numbers.
/// * `name` is the string we get by joining family with everything that's in between
///   family and version with an underscore.
///
/// Returns true if `identifier` is valid and was successfully split
/// into the different components.
///
/// # Examples
///
/// ```ignore
/// let identifier = Token::new("mix_float_2_1");
/// let mut family = Token::default();
/// let mut name = Token::default();
/// let mut version = SdrVersion::default();
///
/// let result = split_shader_identifier(&identifier, &mut family, &mut name, &mut version);
/// assert!(result);
/// assert_eq!(family.as_str(), "mix");
/// assert_eq!(name.as_str(), "mix_float");
/// assert_eq!(version.major(), 2);
/// assert_eq!(version.minor(), 1);
/// ```
pub fn split_shader_identifier(
    identifier: &Token,
    family: &mut Token,
    name: &mut Token,
    version: &mut SdrVersion,
) -> bool {
    let tokens: Vec<&str> = identifier.as_str().split('_').collect();

    if tokens.is_empty() {
        return false;
    }

    *family = Token::new(tokens[0]);

    if tokens.len() == 1 {
        *family = identifier.clone();
        *name = identifier.clone();
        *version = SdrVersion::default();
        return true;
    }

    if tokens.len() == 2 {
        if is_number(tokens[1]) {
            let major = tokens[1].parse::<i32>().unwrap_or(0);
            *version = SdrVersion::new(major, 0);
            *name = family.clone();
        } else {
            *version = SdrVersion::default();
            *name = identifier.clone();
        }
        return true;
    }

    let last_is_number = is_number(tokens[tokens.len() - 1]);
    let penultimate_is_number = is_number(tokens[tokens.len() - 2]);

    if penultimate_is_number {
        if !last_is_number {
            // Invalid: penultimate is number but last is not
            return false;
        }
        // Has major and minor version
        let major = tokens[tokens.len() - 2].parse::<i32>().unwrap_or(0);
        let minor = tokens[tokens.len() - 1].parse::<i32>().unwrap_or(0);
        *version = SdrVersion::new(major, minor);
        *name = Token::new(&tokens[..tokens.len() - 2].join("_"));
    } else if last_is_number {
        // Has just a major version
        let major = tokens[tokens.len() - 1].parse::<i32>().unwrap_or(0);
        *version = SdrVersion::new(major, 0);
        *name = Token::new(&tokens[..tokens.len() - 1].join("_"));
    } else {
        // No version information available
        *name = identifier.clone();
        *version = SdrVersion::default();
    }

    true
}

/// Checks if a string is a valid non-negative integer.
fn is_number(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Returns a vector of discovery results that have been found while walking
/// the given search paths.
///
/// Each path in `search_paths` is walked recursively, optionally following
/// symlinks if `follow_symlinks` is true, looking for files that match one of
/// the provided `allowed_extensions`. These files are represented in the
/// discovery results that are returned.
///
/// The identifier for each discovery result is the base name of the represented
/// file with the extension removed. The `parse_identifier_fn` is used to parse
/// the family, name, and version from the identifier that will set in the
/// file's discovery result. By default, `split_shader_identifier` is used.
///
/// Note that the version for every discovery result returned by this function
/// will be naively marked as being default even if multiple versions with the
/// same name are found.
pub fn discover_shader_nodes(
    search_paths: &SdrStringVec,
    allowed_extensions: &SdrStringVec,
    follow_symlinks: bool,
    source_type: Option<&Token>,
    parse_identifier_fn: Option<&SdrParseIdentifierFn>,
) -> SdrShaderNodeDiscoveryResultVec {
    let mut found_nodes = SdrShaderNodeDiscoveryResultVec::new();
    let mut found_nodes_with_types: SdrStringSet = HashSet::new();

    for search_path in search_paths {
        let path = Path::new(search_path);
        if !path.is_dir() {
            continue;
        }

        walk_directory(
            path,
            &mut found_nodes,
            &mut found_nodes_with_types,
            allowed_extensions,
            source_type,
            parse_identifier_fn,
            follow_symlinks,
        );
    }

    found_nodes
}

/// Returns a vector of discovered URIs (as both the unresolved URI and the
/// resolved URI) that are found while walking the given search paths.
///
/// This is an alternative to `discover_shader_nodes` for discovery plugins
/// that want to search for files that are not meant to be returned by discovery
/// themselves, but can be parsed to generate the discovery results.
pub fn discover_files(
    search_paths: &SdrStringVec,
    allowed_extensions: &SdrStringVec,
    follow_symlinks: bool,
) -> SdrDiscoveryUriVec {
    let mut found_uris = SdrDiscoveryUriVec::new();

    for search_path in search_paths {
        let path = Path::new(search_path);
        if !path.is_dir() {
            continue;
        }

        walk_directory_for_files(path, &mut found_uris, allowed_extensions, follow_symlinks);
    }

    found_uris
}

/// Walks a directory recursively looking for shader files.
fn walk_directory(
    dir_path: &Path,
    found_nodes: &mut SdrShaderNodeDiscoveryResultVec,
    found_nodes_with_types: &mut SdrStringSet,
    allowed_extensions: &SdrStringVec,
    source_type: Option<&Token>,
    parse_identifier_fn: Option<&SdrParseIdentifierFn>,
    follow_symlinks: bool,
) {
    let entries = match std::fs::read_dir(dir_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = if follow_symlinks {
            std::fs::metadata(&path)
        } else {
            std::fs::symlink_metadata(&path)
        };

        let metadata = match metadata {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            walk_directory(
                &path,
                found_nodes,
                found_nodes_with_types,
                allowed_extensions,
                source_type,
                parse_identifier_fn,
                follow_symlinks,
            );
        } else if metadata.is_file() {
            examine_file(
                &path,
                found_nodes,
                found_nodes_with_types,
                allowed_extensions,
                source_type,
                parse_identifier_fn,
            );
        }
    }
}

/// Walks a directory recursively looking for files with matching extensions.
fn walk_directory_for_files(
    dir_path: &Path,
    found_uris: &mut SdrDiscoveryUriVec,
    allowed_extensions: &SdrStringVec,
    follow_symlinks: bool,
) {
    let entries = match std::fs::read_dir(dir_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = if follow_symlinks {
            std::fs::metadata(&path)
        } else {
            std::fs::symlink_metadata(&path)
        };

        let metadata = match metadata {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            walk_directory_for_files(&path, found_uris, allowed_extensions, follow_symlinks);
        } else if metadata.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if allowed_extensions
                    .iter()
                    .any(|e| e.to_lowercase() == ext_lower)
                {
                    let uri = path.to_string_lossy().to_string();
                    // In a full implementation, this would use ar::resolve
                    let resolved_uri = std::fs::canonicalize(&path)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| uri.clone());

                    found_uris.push(SdrDiscoveryUri { uri, resolved_uri });
                }
            }
        }
    }
}

/// Examines a single file for potential shader node.
fn examine_file(
    file_path: &Path,
    found_nodes: &mut SdrShaderNodeDiscoveryResultVec,
    found_nodes_with_types: &mut SdrStringSet,
    allowed_extensions: &SdrStringVec,
    source_type: Option<&Token>,
    parse_identifier_fn: Option<&SdrParseIdentifierFn>,
) {
    // Get extension
    let extension = match file_path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return,
    };

    // Check if extension is allowed
    if !allowed_extensions
        .iter()
        .any(|e| e.to_lowercase() == extension)
    {
        return;
    }

    // Get file name without extension
    let file_stem = match file_path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return,
    };

    let uri = file_path.to_string_lossy().to_string();
    let identifier = Token::new(file_stem);
    let identifier_and_type = format!("{}-{}", identifier.as_str(), extension);

    // Check for duplicates
    if !found_nodes_with_types.insert(identifier_and_type) {
        // Duplicate found, skip
        return;
    }

    // Parse identifier into family, name, version
    let mut family = Token::default();
    let mut name = Token::default();
    let mut version = SdrVersion::default();

    let parsed = if let Some(parse_fn) = parse_identifier_fn {
        parse_fn(&identifier, &mut family, &mut name, &mut version)
    } else {
        split_shader_identifier(&identifier, &mut family, &mut name, &mut version)
    };

    if !parsed {
        // Could not parse identifier, skip
        return;
    }

    // Resolve URI (in full implementation would use ar::resolve)
    let resolved_uri = std::fs::canonicalize(file_path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| uri.clone());

    let discovery_type = Token::new(&extension);
    let src_type = source_type
        .cloned()
        .unwrap_or_else(|| discovery_type.clone());

    let result = SdrShaderNodeDiscoveryResult::minimal(
        identifier,
        version.as_default(), // Mark as default version
        name.as_str().to_string(),
        discovery_type,
        src_type,
        uri,
        resolved_uri,
    );

    // Set family on the result
    let mut result_with_family = result;
    result_with_family.family = family;

    found_nodes.push(result_with_family);
}

/// Returns the extension from a file path (lowercase, without leading dot).
pub fn get_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

/// Returns the file stem (name without extension) from a path.
pub fn get_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_identifier_simple() {
        let id = Token::new("myshader");
        let mut family = Token::default();
        let mut name = Token::default();
        let mut version = SdrVersion::default();

        assert!(split_shader_identifier(
            &id,
            &mut family,
            &mut name,
            &mut version
        ));
        assert_eq!(family.as_str(), "myshader");
        assert_eq!(name.as_str(), "myshader");
        assert!(!version.is_valid());
    }

    #[test]
    fn test_split_identifier_with_major_version() {
        let id = Token::new("mix_float_2");
        let mut family = Token::default();
        let mut name = Token::default();
        let mut version = SdrVersion::default();

        assert!(split_shader_identifier(
            &id,
            &mut family,
            &mut name,
            &mut version
        ));
        assert_eq!(family.as_str(), "mix");
        assert_eq!(name.as_str(), "mix_float");
        assert!(version.is_valid());
        assert_eq!(version.major(), 2);
        assert_eq!(version.minor(), 0);
    }

    #[test]
    fn test_split_identifier_with_major_minor_version() {
        let id = Token::new("mix_float_2_1");
        let mut family = Token::default();
        let mut name = Token::default();
        let mut version = SdrVersion::default();

        assert!(split_shader_identifier(
            &id,
            &mut family,
            &mut name,
            &mut version
        ));
        assert_eq!(family.as_str(), "mix");
        assert_eq!(name.as_str(), "mix_float");
        assert!(version.is_valid());
        assert_eq!(version.major(), 2);
        assert_eq!(version.minor(), 1);
    }

    #[test]
    fn test_split_identifier_no_version() {
        let id = Token::new("mix_float_test");
        let mut family = Token::default();
        let mut name = Token::default();
        let mut version = SdrVersion::default();

        assert!(split_shader_identifier(
            &id,
            &mut family,
            &mut name,
            &mut version
        ));
        assert_eq!(family.as_str(), "mix");
        assert_eq!(name.as_str(), "mix_float_test");
        assert!(!version.is_valid());
    }

    #[test]
    fn test_is_number() {
        assert!(is_number("123"));
        assert!(is_number("0"));
        assert!(!is_number(""));
        assert!(!is_number("12a"));
        assert!(!is_number("abc"));
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("/path/to/file.osl"), "osl");
        assert_eq!(get_extension("/path/to/file.GLSLFX"), "glslfx");
        assert_eq!(get_extension("/path/to/file"), "");
    }

    #[test]
    fn test_get_stem() {
        assert_eq!(get_stem("/path/to/myshader.osl"), "myshader");
        assert_eq!(get_stem("/path/to/myshader"), "myshader");
    }
}
