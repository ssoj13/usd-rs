//! SdfSite - a location in a layer where opinions may be found.
//!
//! An `Site` is a simple representation of a location in a layer. It is
//! a pair of layer handle and path within that layer.
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{Site, Path};
//!
//! // Create a site with just a path (no layer yet)
//! let path = Path::from("/World/Cube");
//! let site = Site::from_path(path.clone());
//!
//! assert!(site.path() == &path);
//! assert!(!site.is_valid()); // No layer
//! ```

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

use super::abstract_data::Value;
use super::{Layer, Path};
use usd_tf::Token;

/// Layer handle that wraps Weak<Layer>.
///
/// Uses a weak reference to avoid circular dependencies and allow layers to be dropped.
#[derive(Debug, Clone)]
pub struct LayerHandle {
    inner: Option<Weak<Layer>>,
}

impl Default for LayerHandle {
    fn default() -> Self {
        Self::null()
    }
}

impl LayerHandle {
    /// Create an empty (null) layer handle.
    #[must_use]
    pub fn null() -> Self {
        Self { inner: None }
    }

    /// Create a layer handle from Arc<Layer>.
    #[must_use]
    pub fn from_layer(layer: &Arc<Layer>) -> Self {
        Self {
            inner: Some(Arc::downgrade(layer)),
        }
    }

    /// Create a layer handle from a Weak<Layer> reference.
    #[must_use]
    pub(crate) fn from_weak(weak: Weak<Layer>) -> Self {
        Self { inner: Some(weak) }
    }

    /// Returns true if this handle points to a valid layer.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.inner.as_ref().and_then(|w| w.upgrade()).is_some()
    }

    /// Returns true if this handle is null/empty.
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.inner.is_none()
    }

    /// Upgrade the weak reference to Arc<Layer>.
    pub fn upgrade(&self) -> Option<Arc<Layer>> {
        self.inner.as_ref().and_then(|w| w.upgrade())
    }

    /// Get prim spec at the given path.
    pub fn get_prim_at_path(&self, path: &Path) -> Option<super::prim_spec::PrimSpec> {
        self.upgrade()?.get_prim_at_path(path)
    }

    /// Get field value.
    pub fn get_field(&self, path: &Path, field_name: &Token) -> Option<Value> {
        let layer = self.upgrade()?;
        let data = layer.data.read().expect("layer data lock poisoned");
        data.get_field(path, field_name)
    }

    /// Set field value.
    pub fn set_field(&self, path: &Path, field_name: &Token, value: Value) -> bool {
        let layer = match self.upgrade() {
            Some(l) => l,
            None => return false,
        };
        let mut data = layer.data.write().expect("layer data lock poisoned");
        data.set_field(path, field_name, value);
        true
    }

    /// Check if field exists.
    pub fn has_field(&self, path: &Path, field_name: &Token) -> bool {
        let layer = match self.upgrade() {
            Some(l) => l,
            None => return false,
        };
        let data = layer.data.read().expect("layer data lock poisoned");
        data.has_field(path, field_name)
    }

    /// Erase field.
    pub fn erase_field(&self, path: &Path, field_name: &Token) -> bool {
        let layer = match self.upgrade() {
            Some(l) => l,
            None => return false,
        };
        let mut data = layer.data.write().expect("layer data lock poisoned");
        data.erase_field(path, field_name);
        true
    }

    /// List all fields at path.
    pub fn list_fields(&self, path: &Path) -> Vec<Token> {
        let layer = match self.upgrade() {
            Some(l) => l,
            None => return Vec::new(),
        };
        let data = layer.data.read().expect("layer data lock poisoned");
        data.list_fields(path)
    }

    /// Create a prim spec at the given path.
    pub fn create_prim_spec(
        &self,
        path: &Path,
        specifier: super::Specifier,
        type_name: &str,
    ) -> Option<super::prim_spec::PrimSpec> {
        self.upgrade()?.create_prim_spec(path, specifier, type_name)
    }

    /// Get property at path.
    pub fn get_property_at_path(&self, path: &Path) -> Option<super::property_spec::PropertySpec> {
        self.upgrade()?.get_property_at_path(path)
    }

    /// Get attribute at path.
    pub fn get_attribute_at_path(&self, path: &Path) -> Option<super::AttributeSpec> {
        self.upgrade()?.get_attribute_at_path(path)
    }

    /// Get relationship at path.
    pub fn get_relationship_at_path(
        &self,
        path: &Path,
    ) -> Option<super::relationship_spec::RelationshipSpec> {
        self.upgrade()?.get_relationship_at_path(path)
    }

    /// Check if spec exists at path.
    pub fn has_spec(&self, path: &Path) -> bool {
        let layer = match self.upgrade() {
            Some(l) => l,
            None => return false,
        };
        let data = layer.data.read().expect("layer data lock poisoned");
        data.has_spec(path)
    }

    /// Get spec type at path.
    pub fn get_spec_type(&self, path: &Path) -> super::types::SpecType {
        let layer = match self.upgrade() {
            Some(l) => l,
            None => return super::types::SpecType::Unknown,
        };
        let data = layer.data.read().expect("layer data lock poisoned");
        data.get_spec_type(path)
    }

    /// Check if layer is read-only.
    pub fn is_read_only(&self) -> bool {
        match self.upgrade() {
            Some(l) => !l.permission_to_edit(),
            None => true,
        }
    }
}

impl PartialEq for LayerHandle {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (Some(a), Some(b)) => Weak::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for LayerHandle {}

impl PartialOrd for LayerHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LayerHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.inner, &other.inner) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => {
                let a_ptr = a.as_ptr() as usize;
                let b_ptr = b.as_ptr() as usize;
                a_ptr.cmp(&b_ptr)
            }
        }
    }
}

impl Hash for LayerHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.inner {
            Some(weak) => (weak.as_ptr() as usize).hash(state),
            None => 0usize.hash(state),
        }
    }
}

/// A location in a layer where opinions may be found.
///
/// `Site` is a simple pair of layer handle and path within that layer.
/// It represents a specific location where scene description data may exist.
///
/// # Validity
///
/// A site is considered valid (converts to `true`) if and only if both:
/// - The layer handle points to a valid layer
/// - The path is not empty
///
/// Note: A valid site does NOT imply that opinions actually exist at that
/// location - it only means the location is well-formed.
#[derive(Debug, Clone, Default)]
pub struct Site {
    /// The layer containing this site.
    pub layer: LayerHandle,
    /// The path within the layer.
    pub path: Path,
}

impl Site {
    /// Create a new site.
    #[must_use]
    pub fn new(layer: LayerHandle, path: Path) -> Self {
        Self { layer, path }
    }

    /// Create a site with only a path (null layer).
    #[must_use]
    pub fn from_path(path: Path) -> Self {
        Self {
            layer: LayerHandle::null(),
            path,
        }
    }

    /// Create an empty site.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Get the layer handle.
    #[must_use]
    pub fn layer(&self) -> &LayerHandle {
        &self.layer
    }

    /// Get the path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns true if this site is valid.
    ///
    /// A site is valid if it has both a valid layer and a non-empty path.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.layer.is_valid() && !self.path.is_empty()
    }
}

impl PartialEq for Site {
    fn eq(&self, other: &Self) -> bool {
        self.layer == other.layer && self.path == other.path
    }
}

impl Eq for Site {}

impl PartialOrd for Site {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Site {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.layer.cmp(&other.layer) {
            Ordering::Equal => self.path.cmp(&other.path),
            ord => ord,
        }
    }
}

impl Hash for Site {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.layer.hash(state);
        self.path.hash(state);
    }
}

/// Explicit bool conversion - true iff both layer and path are valid.
impl From<&Site> for bool {
    fn from(site: &Site) -> bool {
        site.is_valid()
    }
}

/// Type alias for a set of sites.
pub type SiteSet = std::collections::BTreeSet<Site>;

/// Type alias for a vector of sites.
pub type SiteVector = Vec<Site>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_default() {
        let site = Site::default();
        assert!(!site.is_valid());
        assert!(site.layer.is_null());
        assert!(site.path.is_empty());
    }

    #[test]
    fn test_site_from_path() {
        let path = Path::from("/World");
        let site = Site::from_path(path.clone());

        assert_eq!(site.path(), &path);
        assert!(site.layer.is_null());
        assert!(!site.is_valid()); // No layer
    }

    #[test]
    fn test_site_equality() {
        let path1 = Path::from("/World");
        let path2 = Path::from("/World");
        let path3 = Path::from("/Other");

        let site1 = Site::from_path(path1);
        let site2 = Site::from_path(path2);
        let site3 = Site::from_path(path3);

        assert_eq!(site1, site2);
        assert_ne!(site1, site3);
    }

    #[test]
    fn test_site_ordering() {
        let site1 = Site::from_path(Path::from("/A"));
        let site2 = Site::from_path(Path::from("/B"));

        assert!(site1 < site2);
        assert!(site2 > site1);
    }

    #[test]
    fn test_layer_handle_null() {
        let handle = LayerHandle::null();
        assert!(handle.is_null());
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_layer_handle_equality() {
        let h1 = LayerHandle::null();
        let h2 = LayerHandle::null();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_site_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(Site::from_path(Path::from("/World")));
        set.insert(Site::from_path(Path::from("/Other")));
        set.insert(Site::from_path(Path::from("/World"))); // Duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_site_set() {
        let mut set = SiteSet::new();
        set.insert(Site::from_path(Path::from("/B")));
        set.insert(Site::from_path(Path::from("/A")));
        set.insert(Site::from_path(Path::from("/C")));

        let paths: Vec<_> = set.iter().map(|s| s.path().as_str()).collect();
        assert_eq!(paths, vec!["/A", "/B", "/C"]);
    }
}
