//! PCP site types.
//!
//! A site specifies a path in a layer stack of scene description.
//!
//! # Examples
//!
//! ```
//! use usd_pcp::{Site, LayerStackIdentifier};
//! use usd_sdf::Path;
//!
//! let layer_stack = LayerStackIdentifier::new("root.usda");
//! let site = Site::new(layer_stack, Path::absolute_root());
//!
//! assert!(site.is_valid());
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};

use usd_sdf::Path;

use super::LayerStackIdentifier;

/// A site specifies a path in a layer stack of scene description.
///
/// A site is simply a pair of layer stack identifier and path within that
/// layer stack.
///
/// # Examples
///
/// ```
/// use usd_pcp::{Site, LayerStackIdentifier};
/// use usd_sdf::Path;
///
/// let layer_stack = LayerStackIdentifier::new("root.usda");
/// let path = Path::from("/World/Geometry");
/// let site = Site::new(layer_stack, path);
///
/// assert!(site.is_valid());
/// assert!(!site.path.is_empty());
/// ```
#[derive(Clone, Debug, Default)]
pub struct Site {
    /// The layer stack identifier.
    pub layer_stack_identifier: LayerStackIdentifier,
    /// The path within the layer stack.
    pub path: Path,
}

impl Site {
    /// Creates a new site.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{Site, LayerStackIdentifier};
    /// use usd_sdf::Path;
    ///
    /// let site = Site::new(
    ///     LayerStackIdentifier::new("root.usda"),
    ///     Path::from("/World")
    /// );
    /// ```
    #[must_use]
    pub fn new(layer_stack_identifier: LayerStackIdentifier, path: Path) -> Self {
        Self {
            layer_stack_identifier,
            path,
        }
    }

    /// Creates a site from a layer stack and path.
    pub fn from_layer_stack(layer_stack: &super::LayerStackRefPtr, path: Path) -> Self {
        Self {
            layer_stack_identifier: layer_stack.identifier().clone(),
            path,
        }
    }

    /// Creates a site from a layer handle and path.
    pub fn from_layer_handle(layer: &usd_sdf::LayerHandle, path: Path) -> Self {
        if let Some(layer_arc) = layer.upgrade() {
            // Create identifier from layer
            let identifier = super::LayerStackIdentifier::new(layer_arc.identifier());
            Self {
                layer_stack_identifier: identifier,
                path,
            }
        } else {
            Self::default()
        }
    }

    /// Creates a site from a LayerStackSite.
    pub fn from_layer_stack_site(
        layer_stack_site: &super::namespace_edits::LayerStackSite,
    ) -> Self {
        Self {
            layer_stack_identifier: layer_stack_site
                .layer_stack
                .upgrade()
                .map(|ls| ls.identifier().clone())
                .unwrap_or_default(),
            path: layer_stack_site.site_path.clone(),
        }
    }

    /// Returns true if this site is valid.
    ///
    /// A site is valid if both the layer stack identifier and path are valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{Site, LayerStackIdentifier};
    /// use usd_sdf::Path;
    ///
    /// let valid = Site::new(
    ///     LayerStackIdentifier::new("root.usda"),
    ///     Path::from("/World")
    /// );
    /// assert!(valid.is_valid());
    ///
    /// let invalid = Site::new(
    ///     LayerStackIdentifier::new(""),
    ///     Path::empty()
    /// );
    /// assert!(!invalid.is_valid());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.layer_stack_identifier.is_valid() && !self.path.is_empty()
    }
}

impl PartialEq for Site {
    fn eq(&self, other: &Self) -> bool {
        self.layer_stack_identifier == other.layer_stack_identifier && self.path == other.path
    }
}

impl Eq for Site {}

impl Hash for Site {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.layer_stack_identifier.hash(state);
        self.path.hash(state);
    }
}

impl PartialOrd for Site {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Site {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self
            .layer_stack_identifier
            .cmp(&other.layer_stack_identifier)
        {
            std::cmp::Ordering::Equal => self.path.cmp(&other.path),
            ord => ord,
        }
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.layer_stack_identifier, self.path)
    }
}

/// Hash functor for Site.
pub struct SiteHash;

impl SiteHash {
    /// Computes hash for a Site.
    #[must_use]
    pub fn hash(site: &Site) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        site.hash(&mut hasher);
        hasher.finish()
    }
}

/// A set of sites.
pub type SiteSet = std::collections::BTreeSet<Site>;

/// A vector of sites.
pub type SiteVector = Vec<Site>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let site = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        assert!(site.is_valid());
        assert_eq!(
            site.layer_stack_identifier.root_layer.get_authored_path(),
            "root.usda"
        );
        assert_eq!(site.path.as_str(), "/World");
    }

    #[test]
    fn test_is_valid() {
        let valid = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        assert!(valid.is_valid());

        let no_layer = Site::new(LayerStackIdentifier::new(""), Path::from("/World"));
        assert!(!no_layer.is_valid());

        let no_path = Site::new(LayerStackIdentifier::new("root.usda"), Path::empty());
        assert!(!no_path.is_valid());
    }

    #[test]
    fn test_default() {
        let site = Site::default();
        assert!(!site.is_valid());
    }

    #[test]
    fn test_equality() {
        let site1 = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        let site2 = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        let site3 = Site::new(
            LayerStackIdentifier::new("other.usda"),
            Path::from("/World"),
        );
        let site4 = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/Other"));

        assert_eq!(site1, site2);
        assert_ne!(site1, site3);
        assert_ne!(site1, site4);
    }

    #[test]
    fn test_ordering() {
        let site_a = Site::new(LayerStackIdentifier::new("a.usda"), Path::from("/World"));
        let site_b = Site::new(LayerStackIdentifier::new("b.usda"), Path::from("/World"));

        assert!(site_a < site_b);
    }

    #[test]
    fn test_display() {
        let site = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        let s = format!("{}", site);
        assert!(s.contains("root.usda"));
        assert!(s.contains("/World"));
    }

    #[test]
    fn test_hash() {
        let site1 = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        let site2 = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));

        assert_eq!(SiteHash::hash(&site1), SiteHash::hash(&site2));
    }

    #[test]
    fn test_site_set() {
        let mut set = SiteSet::new();
        set.insert(Site::new(
            LayerStackIdentifier::new("a.usda"),
            Path::from("/World"),
        ));
        set.insert(Site::new(
            LayerStackIdentifier::new("b.usda"),
            Path::from("/World"),
        ));
        set.insert(Site::new(
            LayerStackIdentifier::new("a.usda"),
            Path::from("/World"),
        )); // Duplicate

        assert_eq!(set.len(), 2);
    }
}
