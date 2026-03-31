//! Utility functions — name validation, string ops, version, StringResolver.

use std::collections::HashMap;

// UDIM and UV-tile tokens used for texture filename substitution (MaterialX spec).
pub const UDIM_TOKEN: &str = "<UDIM>";
pub const UV_TILE_TOKEN: &str = "<UVTILE>";

/// A helper for applying string modifiers (file prefix, geom prefix, substitutions)
/// to data values in the context of a specific element and geometry.
///
/// Matches C++ `StringResolver` from Element.h / Element.cpp.
/// Construct via `StringResolver::new()` and populate fields before calling `resolve()`.
#[derive(Clone, Debug, Default)]
pub struct StringResolver {
    /// Prefix prepended to resolved filenames.
    pub file_prefix: String,
    /// Prefix prepended to resolved geometry names.
    pub geom_prefix: String,
    /// Substitution map applied to `filename` type values.
    pub filename_map: HashMap<String, String>,
    /// Substitution map applied to `geomname` type values.
    pub geom_name_map: HashMap<String, String>,
}

impl StringResolver {
    pub fn new() -> Self {
        Self::default()
    }

    // --- file prefix ---

    pub fn set_file_prefix(&mut self, prefix: impl Into<String>) {
        self.file_prefix = prefix.into();
    }

    pub fn get_file_prefix(&self) -> &str {
        &self.file_prefix
    }

    // --- geom prefix ---

    pub fn set_geom_prefix(&mut self, prefix: impl Into<String>) {
        self.geom_prefix = prefix.into();
    }

    pub fn get_geom_prefix(&self) -> &str {
        &self.geom_prefix
    }

    // --- filename substitutions ---

    /// Set UDIM token substitution: `<UDIM>` -> `udim`.
    pub fn set_udim_string(&mut self, udim: impl Into<String>) {
        self.filename_map
            .insert(UDIM_TOKEN.to_string(), udim.into());
    }

    /// Set UV-tile token substitution: `<UVTILE>` -> `uv_tile`.
    pub fn set_uv_tile_string(&mut self, uv_tile: impl Into<String>) {
        self.filename_map
            .insert(UV_TILE_TOKEN.to_string(), uv_tile.into());
    }

    /// Set an arbitrary filename substring substitution.
    pub fn set_filename_substitution(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.filename_map.insert(key.into(), value.into());
    }

    pub fn get_filename_substitutions(&self) -> &HashMap<String, String> {
        &self.filename_map
    }

    // --- geometry name substitutions ---

    /// Set an arbitrary geometry name substring substitution.
    pub fn set_geom_name_substitution(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.geom_name_map.insert(key.into(), value.into());
    }

    pub fn get_geom_name_substitutions(&self) -> &HashMap<String, String> {
        &self.geom_name_map
    }

    // --- resolution ---

    /// Apply all modifiers to `input` based on `type_name`.
    ///
    /// - `filename`: prepends `file_prefix`, then applies `filename_map` substitutions.
    /// - `geomname`: prepends `geom_prefix`, then applies `geom_name_map` substitutions.
    /// - Other: returned unchanged.
    pub fn resolve(&self, input: &str, type_name: &str) -> String {
        use crate::core::types::{FILENAME_TYPE_STRING, GEOMNAME_TYPE_STRING};
        if type_name == FILENAME_TYPE_STRING {
            let mut result = input.to_string();
            for (from, to) in &self.filename_map {
                result = result.replace(from.as_str(), to.as_str());
            }
            return format!("{}{}", self.file_prefix, result);
        }
        if type_name == GEOMNAME_TYPE_STRING {
            let mut result = input.to_string();
            for (from, to) in &self.geom_name_map {
                result = result.replace(from.as_str(), to.as_str());
            }
            return format!("{}{}", self.geom_prefix, result);
        }
        input.to_string()
    }

    /// Return true if `type_name` is handled by this resolver (filename or geomname).
    pub fn is_resolved_type(type_name: &str) -> bool {
        use crate::core::types::{FILENAME_TYPE_STRING, GEOMNAME_TYPE_STRING};
        type_name == FILENAME_TYPE_STRING || type_name == GEOMNAME_TYPE_STRING
    }
}

// ─── String utilities ────────────────────────────────────────────────────────

/// Create a valid MaterialX name from the given string.
/// Invalid chars (non-alphanumeric, not '_', not ':') replaced with `replace_char`.
/// Matches MaterialX Util::createValidName.
pub fn create_valid_name(name: &str, replace_char: char) -> String {
    let mut result = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == ':' {
            result.push(c);
        } else {
            result.push(replace_char);
        }
    }
    // Leading digit not allowed
    if let Some(f) = result.chars().next() {
        if f.is_ascii_digit() {
            result.insert(0, replace_char);
        }
    }
    result
}

/// Return true if the given string is a valid MaterialX name.
pub fn is_valid_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    if let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            return false;
        }
        if !c.is_ascii_alphabetic() && c != '_' && c != ':' {
            return false;
        }
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
}

/// Split a string by separators
pub fn split_string(s: &str, sep: &str) -> Vec<String> {
    s.split(|c| sep.contains(c))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Join strings with separator
pub fn join_strings(parts: &[String], sep: &str) -> String {
    parts.join(sep)
}

/// Split name path (e.g. "a/b/c") into vec
pub fn split_name_path(name_path: &str) -> Vec<String> {
    name_path
        .split(NAME_PATH_SEPARATOR)
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Create name path from parts
pub fn create_name_path(parts: &[String]) -> String {
    join_strings(parts, &NAME_PATH_SEPARATOR.to_string())
}

/// Parent of a name path
pub fn parent_name_path(name_path: &str) -> String {
    let parts = split_name_path(name_path);
    if parts.len() <= 1 {
        String::new()
    } else {
        create_name_path(&parts[..parts.len() - 1])
    }
}

/// Trim leading/trailing whitespace
pub fn trim_spaces(s: &str) -> String {
    s.trim().to_string()
}

/// Check if string starts with prefix
pub fn string_starts_with(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

/// Check if string ends with suffix
pub fn string_ends_with(s: &str, suffix: &str) -> bool {
    s.ends_with(suffix)
}

/// Convert to lowercase
pub fn string_to_lower(s: &str) -> String {
    s.to_lowercase()
}

/// Increment the numeric suffix of a name. "testName" -> "testName2", "testName99" -> "testName100".
pub fn increment_name(name: &str) -> String {
    let mut split = name.len();
    let bytes = name.as_bytes();
    while split > 0 && bytes[split - 1].is_ascii_digit() {
        split -= 1;
    }
    if split < name.len() {
        let prefix = &name[..split];
        let suffix = &name[split..];
        if let Ok(n) = suffix.parse::<u64>() {
            return format!("{}{}", prefix, n + 1);
        }
    }
    format!("{}2", name)
}

/// Replace substrings. For each (from, to) pair, replace all occurrences of `from` with `to`.
pub fn replace_substrings(s: &str, replacements: &[(&str, &str)]) -> String {
    let mut result = s.to_string();
    for (from, to) in replacements {
        if from.is_empty() {
            continue;
        }
        result = result.replace(from, to);
    }
    result
}

use crate::core::types::NAME_PATH_SEPARATOR;
