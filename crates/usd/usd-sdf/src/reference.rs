//! Scene description references.
//!
//! A reference is expressed on a prim and identifies another prim in a
//! layer stack, whose opinions will be composed with the referencing prim.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use super::layer_offset::LayerOffset;
use super::path::Path;
// VtValue = usd_vt::Value — arbitrary typed dict values, matching C++ VtDictionary
use usd_vt::Value as VtValue;

/// Represents a reference and all its metadata.
///
/// A reference is expressed on a prim in a given layer and identifies a
/// prim in a layer stack. All opinions under the referenced prim will be
/// composed with opinions under the referencing prim.
///
/// # External vs Internal References
///
/// - **External reference**: Has a non-empty asset path, referring to
///   another layer stack.
/// - **Internal reference**: Has an empty asset path, referring to the
///   same layer stack.
///
/// # Default Prim
///
/// If the prim path is empty, the reference targets the default prim
/// specified in the root layer of the referenced layer stack.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::Reference;
///
/// // External reference to a specific prim
/// let ext_ref = Reference::new(
///     "assets/model.usd",
///     "/Model",
/// );
/// assert!(!ext_ref.is_internal());
///
/// // Internal reference
/// let int_ref = Reference::internal("/SharedPrim");
/// assert!(int_ref.is_internal());
/// ```
#[derive(Clone)]
pub struct Reference {
    /// The asset path to the external layer.
    asset_path: String,
    /// The path to the referenced prim.
    prim_path: Path,
    /// The layer offset to transform time.
    layer_offset: LayerOffset,
    /// Custom data associated with the reference (VtDictionary equivalent).
    custom_data: HashMap<String, VtValue>,
}

impl Default for Reference {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl Reference {
    /// Creates a new reference with the given asset path and prim path.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - Path to the referenced layer (empty for internal)
    /// * `prim_path` - Path to the referenced prim (empty for default prim)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Reference;
    ///
    /// let reference = Reference::new("model.usd", "/Model");
    /// assert_eq!(reference.asset_path(), "model.usd");
    /// ```
    pub fn new(asset_path: impl Into<String>, prim_path: impl AsRef<str>) -> Self {
        let asset = asset_path.into();
        let prim = if prim_path.as_ref().is_empty() {
            Path::empty()
        } else {
            Path::from_string(prim_path.as_ref()).unwrap_or_else(Path::empty)
        };
        Self {
            asset_path: asset,
            prim_path: prim,
            layer_offset: LayerOffset::identity(),
            custom_data: HashMap::new(),
        }
    }

    /// Creates a reference with all metadata.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - Path to the referenced layer
    /// * `prim_path` - Path to the referenced prim
    /// * `layer_offset` - Time offset/scale transformation
    /// * `custom_data` - Custom metadata dictionary
    pub fn with_metadata(
        asset_path: impl Into<String>,
        prim_path: impl AsRef<str>,
        layer_offset: LayerOffset,
        custom_data: HashMap<String, VtValue>,
    ) -> Self {
        let mut reference = Self::new(asset_path, prim_path);
        reference.layer_offset = layer_offset;
        reference.custom_data = custom_data;
        reference
    }

    /// Creates an internal reference to the given prim.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the referenced prim
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Reference;
    ///
    /// let reference = Reference::internal("/SharedPrim");
    /// assert!(reference.is_internal());
    /// ```
    pub fn internal(prim_path: impl AsRef<str>) -> Self {
        Self::new("", prim_path)
    }

    /// Creates a reference to the default prim in the given layer.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - Path to the referenced layer
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Reference;
    ///
    /// let reference = Reference::to_default_prim("model.usd");
    /// assert!(reference.prim_path().is_empty());
    /// ```
    pub fn to_default_prim(asset_path: impl Into<String>) -> Self {
        Self::new(asset_path, "")
    }

    /// Returns the asset path to the root layer of the referenced layer stack.
    ///
    /// This will be empty for internal references.
    pub fn asset_path(&self) -> &str {
        &self.asset_path
    }

    /// Sets the asset path, validating for control characters.
    ///
    /// Per C++ SdfReference::SetAssetPath, routes through SdfAssetPath
    /// to reject illegal control characters. Invalid paths become empty.
    pub fn set_asset_path(&mut self, asset_path: impl Into<String>) {
        let raw = asset_path.into();
        // Validate via AssetPath to reject control characters (C++ uses SdfAssetPath ctor)
        let validated = usd_vt::AssetPath::new(&raw);
        self.asset_path = validated.get_asset_path().to_string();
    }

    /// Returns the path of the referenced prim.
    ///
    /// This will be empty if the reference targets the default prim.
    pub fn prim_path(&self) -> &Path {
        &self.prim_path
    }

    /// Sets the prim path.
    pub fn set_prim_path(&mut self, prim_path: Path) {
        self.prim_path = prim_path;
    }

    /// Returns the layer offset.
    pub fn layer_offset(&self) -> &LayerOffset {
        &self.layer_offset
    }

    /// Sets the layer offset.
    pub fn set_layer_offset(&mut self, layer_offset: LayerOffset) {
        self.layer_offset = layer_offset;
    }

    /// Returns the custom data (VtDictionary equivalent).
    pub fn custom_data(&self) -> &HashMap<String, VtValue> {
        &self.custom_data
    }

    /// Sets the custom data dictionary.
    pub fn set_custom_data(&mut self, custom_data: HashMap<String, VtValue>) {
        self.custom_data = custom_data;
    }

    /// Sets a custom data entry to an arbitrary typed value.
    ///
    /// If value is empty (VtValue::empty()), removes the entry.
    pub fn set_custom_data_entry(&mut self, name: impl Into<String>, value: VtValue) {
        let name = name.into();
        if value.is_empty() {
            self.custom_data.remove(&name);
        } else {
            self.custom_data.insert(name, value);
        }
    }

    /// Returns true if this is an internal reference.
    ///
    /// An internal reference has an empty asset path.
    pub fn is_internal(&self) -> bool {
        self.asset_path.is_empty()
    }

    /// Returns the hash of this reference.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns true if this reference has the same identity as another.
    ///
    /// Identity is determined by asset path and prim path only;
    /// layer offset and custom data are ignored.
    pub fn identity_equal(&self, other: &Reference) -> bool {
        self.asset_path == other.asset_path && self.prim_path == other.prim_path
    }
}

impl PartialEq for Reference {
    fn eq(&self, other: &Self) -> bool {
        self.asset_path == other.asset_path
            && self.prim_path == other.prim_path
            && self.layer_offset == other.layer_offset
            && self.custom_data == other.custom_data
    }
}

impl Eq for Reference {}

impl PartialOrd for Reference {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Reference {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.asset_path.cmp(&other.asset_path) {
            Ordering::Equal => match self.prim_path.cmp(&other.prim_path) {
                Ordering::Equal => self.layer_offset.cmp(&other.layer_offset),
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl Hash for Reference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset_path.hash(state);
        self.prim_path.hash(state);
        self.layer_offset.hash(state);
        // Note: HashMap is not hashable, so we hash the sorted keys/values
        let mut entries: Vec<_> = self.custom_data.iter().collect();
        entries.sort_by_key(|(k, _)| *k);
        for (k, v) in entries {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl fmt::Debug for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reference")
            .field("asset_path", &self.asset_path)
            .field("prim_path", &self.prim_path)
            .field("layer_offset", &self.layer_offset)
            .field("custom_data", &self.custom_data)
            .finish()
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_internal() {
            write!(f, "Reference({})", self.prim_path)
        } else if self.prim_path.is_empty() {
            write!(f, "Reference({})", self.asset_path)
        } else {
            write!(f, "Reference({} {})", self.asset_path, self.prim_path)
        }
    }
}

/// A vector of references.
pub type ReferenceVector = Vec<Reference>;

/// Finds the index of a reference with the same identity as the given reference.
///
/// Identity is determined by asset path and prim path only.
/// Returns -1 if not found.
///
/// # Arguments
///
/// * `references` - The vector of references to search
/// * `reference_id` - The reference to find (by identity)
pub fn find_reference_by_identity(references: &ReferenceVector, reference_id: &Reference) -> i32 {
    for (i, r) in references.iter().enumerate() {
        if r.identity_equal(reference_id) {
            return i as i32;
        }
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let reference = Reference::new("model.usd", "/Model");
        assert_eq!(reference.asset_path(), "model.usd");
        assert_eq!(reference.prim_path().as_str(), "/Model");
    }

    #[test]
    fn test_default() {
        let reference = Reference::default();
        assert!(reference.asset_path().is_empty());
        assert!(reference.prim_path().is_empty());
        assert!(reference.is_internal());
    }

    #[test]
    fn test_internal() {
        let reference = Reference::internal("/SharedPrim");
        assert!(reference.is_internal());
        assert_eq!(reference.prim_path().as_str(), "/SharedPrim");
    }

    #[test]
    fn test_to_default_prim() {
        let reference = Reference::to_default_prim("model.usd");
        assert!(!reference.is_internal());
        assert!(reference.prim_path().is_empty());
    }

    #[test]
    fn test_with_metadata() {
        let mut custom = HashMap::new();
        custom.insert("key".to_string(), VtValue::from("value".to_string()));

        let reference =
            Reference::with_metadata("model.usd", "/Model", LayerOffset::new(10.0, 2.0), custom);

        assert_eq!(reference.layer_offset().offset(), 10.0);
        // custom_data stores VtValue, check it's present
        assert!(reference.custom_data().contains_key("key"));
    }

    #[test]
    fn test_setters() {
        let mut reference = Reference::default();
        reference.set_asset_path("model.usd");
        reference.set_prim_path(Path::from_string("/Model").unwrap());
        reference.set_layer_offset(LayerOffset::new(5.0, 1.0));

        assert_eq!(reference.asset_path(), "model.usd");
        assert_eq!(reference.prim_path().as_str(), "/Model");
        assert_eq!(reference.layer_offset().offset(), 5.0);
    }

    #[test]
    fn test_custom_data_entry() {
        let mut reference = Reference::default();
        reference.set_custom_data_entry("key", VtValue::from("value".to_string()));
        assert!(reference.custom_data().contains_key("key"));

        // Empty VtValue removes entry
        reference.set_custom_data_entry("key", VtValue::empty());
        assert!(reference.custom_data().get("key").is_none());
    }

    #[test]
    fn test_equality() {
        let r1 = Reference::new("model.usd", "/Model");
        let r2 = Reference::new("model.usd", "/Model");
        let r3 = Reference::new("other.usd", "/Model");

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn test_identity_equal() {
        let r1 = Reference::with_metadata(
            "model.usd",
            "/Model",
            LayerOffset::new(10.0, 1.0),
            HashMap::new(),
        );
        let r2 = Reference::with_metadata(
            "model.usd",
            "/Model",
            LayerOffset::new(20.0, 2.0),
            HashMap::new(),
        );

        // Different layer offsets but same identity
        assert!(r1.identity_equal(&r2));
        assert_ne!(r1, r2); // But not fully equal
    }

    #[test]
    fn test_ordering() {
        let r1 = Reference::new("a.usd", "/Model");
        let r2 = Reference::new("b.usd", "/Model");

        assert!(r1 < r2);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let r1 = Reference::new("model.usd", "/Model");
        let r2 = Reference::new("model.usd", "/Model");
        let r3 = Reference::new("other.usd", "/Model");

        let mut set = HashSet::new();
        set.insert(r1.clone());
        assert!(set.contains(&r2));
        assert!(!set.contains(&r3));
    }

    #[test]
    fn test_display() {
        let external = Reference::new("model.usd", "/Model");
        assert!(format!("{}", external).contains("model.usd"));
        assert!(format!("{}", external).contains("/Model"));

        let internal = Reference::internal("/SharedPrim");
        assert!(format!("{}", internal).contains("/SharedPrim"));

        let default_prim = Reference::to_default_prim("model.usd");
        assert!(format!("{}", default_prim).contains("model.usd"));
    }

    #[test]
    fn test_find_reference_by_identity() {
        let references = vec![
            Reference::new("a.usd", "/A"),
            Reference::new("b.usd", "/B"),
            Reference::new("c.usd", "/C"),
        ];

        let found = find_reference_by_identity(
            &references,
            &Reference::with_metadata("b.usd", "/B", LayerOffset::new(10.0, 1.0), HashMap::new()),
        );
        assert_eq!(found, 1);

        let not_found = find_reference_by_identity(&references, &Reference::new("d.usd", "/D"));
        assert_eq!(not_found, -1);
    }
}
