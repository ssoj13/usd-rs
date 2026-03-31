//! Asset path type, moved here from usd-sdf to break circular dependency.
//!
//! `AssetPath` represents a reference to an external asset (a USD file, image,
//! or other resource). It contains an authored path and optional evaluated and
//! resolved paths. This corresponds to C++ `SdfAssetPath`.
//!
//! The utility functions that anchor/resolve paths against a Layer remain in
//! `usd-sdf`, since they depend on `Layer` and the asset resolution system.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Contains an asset path and optional evaluated and resolved paths.
///
/// When this class is used to author scene description, the value returned
/// by `get_asset_path()` is serialized out; all other fields are ignored.
///
/// Asset paths may contain non-control UTF-8 encoded characters.
/// Specifically, U+0000..U+001F (C0 controls), U+007F (delete),
/// and U+0080..U+009F (C1 controls) are disallowed.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPath {
    /// Raw path, as authored in the layer.
    pub(crate) authored_path: String,
    /// Evaluated authored path (populated when variable expressions are present).
    pub(crate) evaluated_path: String,
    /// Fully evaluated and resolved path.
    pub(crate) resolved_path: String,
}

/// Builder pattern helper for constructing `AssetPath` instances.
#[derive(Clone, Default)]
pub struct AssetPathParams {
    pub(crate) authored_path: String,
    pub(crate) evaluated_path: String,
    pub(crate) resolved_path: String,
}

impl AssetPathParams {
    /// Creates a new empty `AssetPathParams`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the authored path.
    pub fn authored(mut self, path: impl Into<String>) -> Self {
        self.authored_path = path.into();
        self
    }

    /// Sets the evaluated path.
    pub fn evaluated(mut self, path: impl Into<String>) -> Self {
        self.evaluated_path = path.into();
        self
    }

    /// Sets the resolved path.
    pub fn resolved(mut self, path: impl Into<String>) -> Self {
        self.resolved_path = path.into();
        self
    }
}

impl AssetPath {
    /// Constructs an empty asset path.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Constructs an asset path with the given authored path.
    pub fn new(authored_path: impl Into<String>) -> Self {
        let authored = authored_path.into();
        if !Self::is_valid_path(&authored) {
            return Self::default();
        }
        Self {
            authored_path: authored,
            evaluated_path: String::new(),
            resolved_path: String::new(),
        }
    }

    /// Constructs an asset path with authored and resolved paths.
    pub fn with_resolved(
        authored_path: impl Into<String>,
        resolved_path: impl Into<String>,
    ) -> Self {
        let authored = authored_path.into();
        let resolved = resolved_path.into();
        if !Self::is_valid_path(&authored) || !Self::is_valid_path(&resolved) {
            return Self::default();
        }
        Self {
            authored_path: authored,
            evaluated_path: String::new(),
            resolved_path: resolved,
        }
    }

    /// Constructs an asset path from `AssetPathParams`.
    pub fn from_params(params: AssetPathParams) -> Self {
        if !Self::is_valid_path(&params.authored_path)
            || !Self::is_valid_path(&params.evaluated_path)
            || !Self::is_valid_path(&params.resolved_path)
        {
            return Self::default();
        }
        Self {
            authored_path: params.authored_path,
            evaluated_path: params.evaluated_path,
            resolved_path: params.resolved_path,
        }
    }

    /// Checks if a path string is valid (no control characters).
    fn is_valid_path(path: &str) -> bool {
        if path.is_empty() {
            return true;
        }
        for ch in path.chars() {
            let code = ch as u32;
            if code <= 0x1F || code == 0x7F || (0x80..=0x9F).contains(&code) {
                return false;
            }
        }
        true
    }

    /// Returns the asset path as it was authored in the original layer.
    pub fn get_authored_path(&self) -> &str {
        &self.authored_path
    }

    /// Returns the evaluated asset path, if any.
    ///
    /// Empty if the authored path contained no expression variables.
    pub fn get_evaluated_path(&self) -> &str {
        &self.evaluated_path
    }

    /// Returns the asset path used for resolution.
    ///
    /// Returns the evaluated path if present, otherwise the authored path.
    pub fn get_asset_path(&self) -> &str {
        if self.evaluated_path.is_empty() {
            &self.authored_path
        } else {
            &self.evaluated_path
        }
    }

    /// Returns the resolved asset path, if any.
    pub fn get_resolved_path(&self) -> &str {
        &self.resolved_path
    }

    /// Sets the authored path.
    pub fn set_authored_path(&mut self, path: impl Into<String>) {
        self.authored_path = path.into();
    }

    /// Sets the evaluated path.
    pub fn set_evaluated_path(&mut self, path: impl Into<String>) {
        self.evaluated_path = path.into();
    }

    /// Sets the resolved path.
    pub fn set_resolved_path(&mut self, path: impl Into<String>) {
        self.resolved_path = path.into();
    }

    /// Returns the hash of this asset path.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns true if this asset path is empty.
    pub fn is_empty(&self) -> bool {
        self.authored_path.is_empty()
            && self.evaluated_path.is_empty()
            && self.resolved_path.is_empty()
    }
}

impl fmt::Debug for AssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetPath")
            .field("authored_path", &self.authored_path)
            .field("evaluated_path", &self.evaluated_path)
            .field("resolved_path", &self.resolved_path)
            .finish()
    }
}

impl fmt::Display for AssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_asset_path())
    }
}

impl From<&str> for AssetPath {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl From<String> for AssetPath {
    fn from(path: String) -> Self {
        Self::new(path)
    }
}

impl AsRef<str> for AssetPath {
    fn as_ref(&self) -> &str {
        self.get_asset_path()
    }
}

/// Hash function object for `AssetPath` (compatibility with C++ API).
pub struct AssetPathHash;

impl AssetPathHash {
    /// Returns the hash of the given asset path.
    pub fn hash(ap: &AssetPath) -> u64 {
        ap.get_hash()
    }
}

impl std::hash::BuildHasher for AssetPathHash {
    type Hasher = std::collections::hash_map::DefaultHasher;

    fn build_hasher(&self) -> Self::Hasher {
        std::collections::hash_map::DefaultHasher::new()
    }
}

/// Swaps two asset paths.
pub fn swap_asset_paths(lhs: &mut AssetPath, rhs: &mut AssetPath) {
    std::mem::swap(lhs, rhs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let path = AssetPath::empty();
        assert!(path.get_authored_path().is_empty());
        assert!(path.get_evaluated_path().is_empty());
        assert!(path.get_resolved_path().is_empty());
        assert!(path.is_empty());
    }

    #[test]
    fn test_new() {
        let path = AssetPath::new("model.usd");
        assert_eq!(path.get_authored_path(), "model.usd");
        assert!(path.get_evaluated_path().is_empty());
        assert!(path.get_resolved_path().is_empty());
        assert!(!path.is_empty());
    }

    #[test]
    fn test_with_resolved() {
        let path = AssetPath::with_resolved("model.usd", "/root/model.usd");
        assert_eq!(path.get_authored_path(), "model.usd");
        assert_eq!(path.get_resolved_path(), "/root/model.usd");
    }

    #[test]
    fn test_get_asset_path_prefers_evaluated() {
        let path = AssetPath::from_params(
            AssetPathParams::new()
                .authored("model_{VAR}.usd")
                .evaluated("model_a.usd"),
        );
        assert_eq!(path.get_asset_path(), "model_a.usd");
    }

    #[test]
    fn test_equality() {
        let path1 = AssetPath::new("model.usd");
        let path2 = AssetPath::new("model.usd");
        let path3 = AssetPath::new("other.usd");
        assert_eq!(path1, path2);
        assert_ne!(path1, path3);
    }

    #[test]
    fn test_ordering() {
        let path1 = AssetPath::new("a.usd");
        let path2 = AssetPath::new("b.usd");
        assert!(path1 < path2);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;
        let path1 = AssetPath::new("model.usd");
        let path2 = AssetPath::new("model.usd");
        let path3 = AssetPath::new("other.usd");
        let mut set = HashSet::new();
        set.insert(path1.clone());
        assert!(set.contains(&path2));
        assert!(!set.contains(&path3));
    }

    #[test]
    fn test_display() {
        let path = AssetPath::new("model.usd");
        assert_eq!(format!("{}", path), "model.usd");
    }

    #[test]
    fn test_invalid_control_chars() {
        // Control chars should produce empty path
        let path = AssetPath::new("bad\x01path.usd");
        assert!(path.is_empty());
    }
}
