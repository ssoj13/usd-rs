//! Resolved asset path type.
//!
//! A resolved path represents the final, physical location of an asset
//! after resolution has been performed.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Represents a resolved asset path.
///
/// A resolved path is the physical location of an asset after resolution.
/// An empty resolved path indicates that the asset could not be resolved.
///
/// # Examples
///
/// ```
/// use usd_ar::ResolvedPath;
///
/// let path = ResolvedPath::new("/path/to/asset.usd");
/// assert!(!path.is_empty());
/// assert_eq!(path.as_str(), "/path/to/asset.usd");
///
/// let empty = ResolvedPath::empty();
/// assert!(empty.is_empty());
/// ```
#[derive(Clone, Default)]
pub struct ResolvedPath {
    /// The resolved path string.
    path: String,
}

impl ResolvedPath {
    /// Creates a new resolved path from the given path string.
    ///
    /// # Arguments
    ///
    /// * `path` - The resolved path string
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolvedPath;
    ///
    /// let path = ResolvedPath::new("/path/to/asset.usd");
    /// assert_eq!(path.as_str(), "/path/to/asset.usd");
    /// ```
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Creates an empty resolved path.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolvedPath;
    ///
    /// let path = ResolvedPath::empty();
    /// assert!(path.is_empty());
    /// ```
    pub fn empty() -> Self {
        Self {
            path: String::new(),
        }
    }

    /// Returns `true` if this resolved path is empty.
    ///
    /// An empty resolved path indicates that the asset could not be resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolvedPath;
    ///
    /// let path = ResolvedPath::new("/path/to/asset.usd");
    /// assert!(!path.is_empty());
    ///
    /// let empty = ResolvedPath::empty();
    /// assert!(empty.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// Returns the resolved path as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolvedPath;
    ///
    /// let path = ResolvedPath::new("/path/to/asset.usd");
    /// assert_eq!(path.as_str(), "/path/to/asset.usd");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Returns the resolved path as a `Path` reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolvedPath;
    /// use std::path::Path;
    ///
    /// let path = ResolvedPath::new("/path/to/asset.usd");
    /// assert_eq!(path.as_path(), Path::new("/path/to/asset.usd"));
    /// ```
    pub fn as_path(&self) -> &Path {
        Path::new(&self.path)
    }

    /// Converts the resolved path into a `PathBuf`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolvedPath;
    /// use std::path::PathBuf;
    ///
    /// let path = ResolvedPath::new("/path/to/asset.usd");
    /// let pathbuf: PathBuf = path.into_pathbuf();
    /// assert_eq!(pathbuf, PathBuf::from("/path/to/asset.usd"));
    /// ```
    pub fn into_pathbuf(self) -> PathBuf {
        PathBuf::from(self.path)
    }

    /// Consumes the resolved path and returns the underlying string.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolvedPath;
    ///
    /// let path = ResolvedPath::new("/path/to/asset.usd");
    /// let s: String = path.into_string();
    /// assert_eq!(s, "/path/to/asset.usd");
    /// ```
    pub fn into_string(self) -> String {
        self.path
    }

    /// Returns the hash value for this resolved path.
    pub fn hash_value(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.path.hash(&mut hasher);
        hasher.finish()
    }
}

impl fmt::Debug for ResolvedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ResolvedPath").field(&self.path).finish()
    }
}

impl fmt::Display for ResolvedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl Hash for ResolvedPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl PartialEq for ResolvedPath {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for ResolvedPath {}

impl PartialOrd for ResolvedPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResolvedPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}

// String comparison operators (equality + ordering)
impl PartialEq<str> for ResolvedPath {
    fn eq(&self, other: &str) -> bool {
        self.path == other
    }
}

impl PartialEq<&str> for ResolvedPath {
    fn eq(&self, other: &&str) -> bool {
        self.path == *other
    }
}

impl PartialEq<String> for ResolvedPath {
    fn eq(&self, other: &String) -> bool {
        &self.path == other
    }
}

impl PartialEq<ResolvedPath> for str {
    fn eq(&self, other: &ResolvedPath) -> bool {
        self == other.path
    }
}

impl PartialEq<ResolvedPath> for String {
    fn eq(&self, other: &ResolvedPath) -> bool {
        self == &other.path
    }
}

impl PartialOrd<str> for ResolvedPath {
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        self.path.as_str().partial_cmp(other)
    }
}

impl PartialOrd<&str> for ResolvedPath {
    fn partial_cmp(&self, other: &&str) -> Option<Ordering> {
        self.path.as_str().partial_cmp(*other)
    }
}

impl PartialOrd<String> for ResolvedPath {
    fn partial_cmp(&self, other: &String) -> Option<Ordering> {
        self.path.partial_cmp(other)
    }
}

impl PartialOrd<ResolvedPath> for str {
    fn partial_cmp(&self, other: &ResolvedPath) -> Option<Ordering> {
        self.partial_cmp(other.path.as_str())
    }
}

impl PartialOrd<ResolvedPath> for String {
    fn partial_cmp(&self, other: &ResolvedPath) -> Option<Ordering> {
        self.partial_cmp(&other.path)
    }
}

// Conversion implementations
impl From<String> for ResolvedPath {
    fn from(path: String) -> Self {
        Self::new(path)
    }
}

impl From<&str> for ResolvedPath {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl From<PathBuf> for ResolvedPath {
    fn from(path: PathBuf) -> Self {
        Self::new(path.to_string_lossy().into_owned())
    }
}

impl From<&Path> for ResolvedPath {
    fn from(path: &Path) -> Self {
        Self::new(path.to_string_lossy().into_owned())
    }
}

impl From<Cow<'_, str>> for ResolvedPath {
    fn from(path: Cow<'_, str>) -> Self {
        Self::new(path.into_owned())
    }
}

impl AsRef<str> for ResolvedPath {
    fn as_ref(&self) -> &str {
        &self.path
    }
}

impl AsRef<Path> for ResolvedPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

// Boolean conversion - true if non-empty
impl From<&ResolvedPath> for bool {
    fn from(path: &ResolvedPath) -> bool {
        !path.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        assert_eq!(path.as_str(), "/path/to/asset.usd");
        assert!(!path.is_empty());
    }

    #[test]
    fn test_empty() {
        let path = ResolvedPath::empty();
        assert!(path.is_empty());
        assert_eq!(path.as_str(), "");
    }

    #[test]
    fn test_default() {
        let path = ResolvedPath::default();
        assert!(path.is_empty());
    }

    #[test]
    fn test_equality() {
        let path1 = ResolvedPath::new("/path/to/asset.usd");
        let path2 = ResolvedPath::new("/path/to/asset.usd");
        let path3 = ResolvedPath::new("/other/path.usd");

        assert_eq!(path1, path2);
        assert_ne!(path1, path3);
    }

    #[test]
    fn test_string_equality() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        assert!(path == "/path/to/asset.usd");
        assert!(path == String::from("/path/to/asset.usd"));
    }

    #[test]
    fn test_ordering() {
        let path1 = ResolvedPath::new("/a/path.usd");
        let path2 = ResolvedPath::new("/b/path.usd");
        let path3 = ResolvedPath::new("/a/path.usd");

        assert!(path1 < path2);
        assert!(path2 > path1);
        assert!(path1 <= path3);
        assert!(path1 >= path3);
    }

    #[test]
    fn test_string_ordering() {
        let path = ResolvedPath::new("/b/path.usd");

        // PartialOrd<str>
        assert!(path > *"/a/path.usd");
        assert!(path < *"/c/path.usd");

        // PartialOrd<String>
        assert!(path > String::from("/a/path.usd"));
        assert!(path < String::from("/c/path.usd"));

        // Reverse direction: str/String vs ResolvedPath
        assert!(*"/a/path.usd" < path);
        assert!(String::from("/c/path.usd") > path);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let path1 = ResolvedPath::new("/path/to/asset.usd");
        let path2 = ResolvedPath::new("/path/to/asset.usd");

        let mut set = HashSet::new();
        set.insert(path1.clone());
        assert!(set.contains(&path2));
    }

    #[test]
    fn test_from_string() {
        let path: ResolvedPath = String::from("/path/to/asset.usd").into();
        assert_eq!(path.as_str(), "/path/to/asset.usd");
    }

    #[test]
    fn test_from_str() {
        let path: ResolvedPath = "/path/to/asset.usd".into();
        assert_eq!(path.as_str(), "/path/to/asset.usd");
    }

    #[test]
    fn test_from_pathbuf() {
        let pathbuf = PathBuf::from("/path/to/asset.usd");
        let path: ResolvedPath = pathbuf.into();
        assert_eq!(path.as_str(), "/path/to/asset.usd");
    }

    #[test]
    fn test_as_path() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        assert_eq!(path.as_path(), Path::new("/path/to/asset.usd"));
    }

    #[test]
    fn test_into_pathbuf() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        let pathbuf = path.into_pathbuf();
        assert_eq!(pathbuf, PathBuf::from("/path/to/asset.usd"));
    }

    #[test]
    fn test_into_string() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        let s = path.into_string();
        assert_eq!(s, "/path/to/asset.usd");
    }

    #[test]
    fn test_display() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        assert_eq!(format!("{}", path), "/path/to/asset.usd");
    }

    #[test]
    fn test_debug() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        let debug = format!("{:?}", path);
        assert!(debug.contains("ResolvedPath"));
        assert!(debug.contains("/path/to/asset.usd"));
    }

    #[test]
    fn test_clone() {
        let path1 = ResolvedPath::new("/path/to/asset.usd");
        let path2 = path1.clone();
        assert_eq!(path1, path2);
    }

    #[test]
    fn test_bool_conversion() {
        let path = ResolvedPath::new("/path/to/asset.usd");
        assert!(bool::from(&path));

        let empty = ResolvedPath::empty();
        assert!(!bool::from(&empty));
    }
}
