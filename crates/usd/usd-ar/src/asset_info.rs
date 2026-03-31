//! Asset metadata information.
//!
//! Contains information about a resolved asset such as version,
//! name, and resolver-specific metadata.

use std::fmt;
use std::hash::{Hash, Hasher};

use usd_vt::Value;

/// Contains information about a resolved asset.
///
/// This struct holds metadata about an asset that may be populated
/// by the asset resolver during resolution.
///
/// # Examples
///
/// ```
/// use usd_ar::AssetInfo;
///
/// let mut info = AssetInfo::new();
/// info.version = Some("1.0".to_string());
/// info.asset_name = Some("my_asset".to_string());
///
/// assert_eq!(info.version, Some("1.0".to_string()));
/// ```
#[derive(Clone, Default)]
pub struct AssetInfo {
    /// Version of the resolved asset, if any.
    pub version: Option<String>,

    /// The name of the asset represented by the resolved asset, if any.
    pub asset_name: Option<String>,

    /// The repository path corresponding to the resolved asset.
    /// Deprecated but maintained for compatibility.
    pub repo_path: Option<String>,

    /// Additional information specific to the active plugin
    /// asset resolver implementation.
    pub resolver_info: Option<Value>,
}

impl AssetInfo {
    /// Creates a new empty `AssetInfo`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::AssetInfo;
    ///
    /// let info = AssetInfo::new();
    /// assert!(info.version.is_none());
    /// assert!(info.asset_name.is_none());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an `AssetInfo` with the given version.
    ///
    /// # Arguments
    ///
    /// * `version` - The asset version string
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::AssetInfo;
    ///
    /// let info = AssetInfo::with_version("1.0");
    /// assert_eq!(info.version, Some("1.0".to_string()));
    /// ```
    pub fn with_version(version: impl Into<String>) -> Self {
        Self {
            version: Some(version.into()),
            ..Default::default()
        }
    }

    /// Creates an `AssetInfo` with the given asset name.
    ///
    /// # Arguments
    ///
    /// * `name` - The asset name string
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::AssetInfo;
    ///
    /// let info = AssetInfo::with_name("my_asset");
    /// assert_eq!(info.asset_name, Some("my_asset".to_string()));
    /// ```
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            asset_name: Some(name.into()),
            ..Default::default()
        }
    }

    /// Returns `true` if this `AssetInfo` has no data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::AssetInfo;
    ///
    /// let empty = AssetInfo::new();
    /// assert!(empty.is_empty());
    ///
    /// let with_version = AssetInfo::with_version("1.0");
    /// assert!(!with_version.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.version.is_none()
            && self.asset_name.is_none()
            && self.repo_path.is_none()
            && self.resolver_info.is_none()
    }

    /// Swaps the contents of this `AssetInfo` with another.
    ///
    /// # Arguments
    ///
    /// * `other` - The other `AssetInfo` to swap with
    pub fn swap(&mut self, other: &mut AssetInfo) {
        std::mem::swap(&mut self.version, &mut other.version);
        std::mem::swap(&mut self.asset_name, &mut other.asset_name);
        std::mem::swap(&mut self.repo_path, &mut other.repo_path);
        std::mem::swap(&mut self.resolver_info, &mut other.resolver_info);
    }
}

impl fmt::Debug for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetInfo")
            .field("version", &self.version)
            .field("asset_name", &self.asset_name)
            .field("repo_path", &self.repo_path)
            .field("resolver_info", &self.resolver_info.is_some())
            .finish()
    }
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(ref name) = self.asset_name {
            parts.push(format!("name={}", name));
        }
        if let Some(ref version) = self.version {
            parts.push(format!("version={}", version));
        }
        if let Some(ref repo) = self.repo_path {
            parts.push(format!("repo={}", repo));
        }
        if self.resolver_info.is_some() {
            parts.push("resolver_info=<present>".to_string());
        }
        if parts.is_empty() {
            write!(f, "AssetInfo(empty)")
        } else {
            write!(f, "AssetInfo({})", parts.join(", "))
        }
    }
}

impl PartialEq for AssetInfo {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.asset_name == other.asset_name
            && self.repo_path == other.repo_path
            && self.resolver_info == other.resolver_info
    }
}

impl Eq for AssetInfo {}

impl Hash for AssetInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.asset_name.hash(state);
        self.repo_path.hash(state);
        // Hash full resolverInfo value using Value's Hash implementation.
        // This matches C++ TfHashAppend behavior which hashes the full VtValue.
        self.resolver_info.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let info = AssetInfo::new();
        assert!(info.version.is_none());
        assert!(info.asset_name.is_none());
        assert!(info.repo_path.is_none());
        assert!(info.resolver_info.is_none());
    }

    #[test]
    fn test_default() {
        let info = AssetInfo::default();
        assert!(info.is_empty());
    }

    #[test]
    fn test_with_version() {
        let info = AssetInfo::with_version("1.0.0");
        assert_eq!(info.version, Some("1.0.0".to_string()));
        assert!(info.asset_name.is_none());
    }

    #[test]
    fn test_with_name() {
        let info = AssetInfo::with_name("my_asset");
        assert_eq!(info.asset_name, Some("my_asset".to_string()));
        assert!(info.version.is_none());
    }

    #[test]
    fn test_is_empty() {
        let empty = AssetInfo::new();
        assert!(empty.is_empty());

        let mut non_empty = AssetInfo::new();
        non_empty.version = Some("1.0".to_string());
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_equality() {
        let info1 = AssetInfo::with_version("1.0");
        let info2 = AssetInfo::with_version("1.0");
        let info3 = AssetInfo::with_version("2.0");

        assert_eq!(info1, info2);
        assert_ne!(info1, info3);
    }

    #[test]
    fn test_swap() {
        let mut info1 = AssetInfo::with_version("1.0");
        let mut info2 = AssetInfo::with_name("asset");

        info1.swap(&mut info2);

        assert_eq!(info1.asset_name, Some("asset".to_string()));
        assert!(info1.version.is_none());
        assert_eq!(info2.version, Some("1.0".to_string()));
        assert!(info2.asset_name.is_none());
    }

    #[test]
    fn test_clone() {
        let info1 = AssetInfo::with_version("1.0");
        let info2 = info1.clone();
        assert_eq!(info1, info2);
    }

    #[test]
    fn test_display_empty() {
        let info = AssetInfo::new();
        assert_eq!(format!("{}", info), "AssetInfo(empty)");
    }

    #[test]
    fn test_display_with_data() {
        let mut info = AssetInfo::new();
        info.version = Some("1.0".to_string());
        info.asset_name = Some("asset".to_string());

        let display = format!("{}", info);
        assert!(display.contains("name=asset"));
        assert!(display.contains("version=1.0"));
    }

    #[test]
    fn test_debug() {
        let info = AssetInfo::with_version("1.0");
        let debug = format!("{:?}", info);
        assert!(debug.contains("AssetInfo"));
        assert!(debug.contains("1.0"));
    }

    #[test]
    fn test_hash() {
        use std::collections::hash_map::DefaultHasher;

        let info1 = AssetInfo::with_version("1.0");
        let info2 = AssetInfo::with_version("1.0");

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        info1.hash(&mut hasher1);
        info2.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_all_fields() {
        let mut info = AssetInfo::new();
        info.version = Some("1.0".to_string());
        info.asset_name = Some("my_asset".to_string());
        info.repo_path = Some("/repo/path".to_string());
        info.resolver_info = Some(Value::from(42));

        assert!(!info.is_empty());
        assert_eq!(info.version, Some("1.0".to_string()));
        assert_eq!(info.asset_name, Some("my_asset".to_string()));
        assert_eq!(info.repo_path, Some("/repo/path".to_string()));
        assert!(info.resolver_info.is_some());
    }
}
