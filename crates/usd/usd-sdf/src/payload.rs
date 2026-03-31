//! Scene description payloads.
//!
//! A payload represents a prim reference to an external layer that is
//! explicitly loaded by the user. Unlike references, payloads provide
//! a boundary that lazy composition will not traverse across.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use super::layer_offset::LayerOffset;
use super::path::Path;

/// Represents a payload and all its metadata.
///
/// A payload represents a prim reference to an external layer. A payload
/// is similar to a prim reference (see [`Reference`]) with the major
/// difference that payloads are explicitly loaded by the user.
///
/// Unloaded payloads represent a boundary that lazy composition and
/// system behaviors will not traverse across, providing a user-visible
/// way to manage the working set of the scene.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::Payload;
///
/// // Create an external payload
/// let payload = Payload::new("assets/model.usd", "/Model");
/// assert!(!payload.is_internal());
///
/// // Create an internal payload
/// let internal = Payload::internal("/SharedPrim");
/// assert!(internal.is_internal());
/// ```
#[derive(Clone)]
pub struct Payload {
    /// The asset path to the external layer.
    asset_path: String,
    /// The path to the referenced prim.
    prim_path: Path,
    /// The layer offset to transform time.
    layer_offset: LayerOffset,
}

impl Default for Payload {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl Payload {
    /// Creates a new payload with the given asset path and prim path.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - Path to the external layer (empty for internal)
    /// * `prim_path` - Path to the referenced prim (empty for default prim)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Payload;
    ///
    /// let payload = Payload::new("model.usd", "/Model");
    /// assert_eq!(payload.asset_path(), "model.usd");
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
        }
    }

    /// Creates a payload with a layer offset.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - Path to the external layer
    /// * `prim_path` - Path to the referenced prim
    /// * `layer_offset` - Time offset/scale transformation
    pub fn with_layer_offset(
        asset_path: impl Into<String>,
        prim_path: impl AsRef<str>,
        layer_offset: LayerOffset,
    ) -> Self {
        let mut payload = Self::new(asset_path, prim_path);
        payload.layer_offset = layer_offset;
        payload
    }

    /// Creates an internal payload to the given prim.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the referenced prim
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Payload;
    ///
    /// let payload = Payload::internal("/SharedPrim");
    /// assert!(payload.is_internal());
    /// ```
    pub fn internal(prim_path: impl AsRef<str>) -> Self {
        Self::new("", prim_path)
    }

    /// Creates a payload to the default prim in the given layer.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - Path to the external layer
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Payload;
    ///
    /// let payload = Payload::to_default_prim("model.usd");
    /// assert!(payload.prim_path().is_empty());
    /// ```
    pub fn to_default_prim(asset_path: impl Into<String>) -> Self {
        Self::new(asset_path, "")
    }

    /// Returns the asset path to the root layer of the referenced layer stack.
    ///
    /// This will be empty for internal payloads.
    pub fn asset_path(&self) -> &str {
        &self.asset_path
    }

    /// Sets the asset path, validating for control characters.
    ///
    /// Per C++ SdfPayload::SetAssetPath, routes through SdfAssetPath
    /// to reject illegal control characters. Invalid paths become empty.
    pub fn set_asset_path(&mut self, asset_path: impl Into<String>) {
        let raw = asset_path.into();
        // Validate via AssetPath to reject control characters (C++ uses SdfAssetPath ctor)
        let validated = usd_vt::AssetPath::new(&raw);
        self.asset_path = validated.get_asset_path().to_string();
    }

    /// Returns the path of the referenced prim.
    ///
    /// This will be empty if the payload targets the default prim.
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

    /// Returns true if this is an internal payload.
    ///
    /// An internal payload has an empty asset path.
    pub fn is_internal(&self) -> bool {
        self.asset_path.is_empty()
    }

    /// Returns the hash of this payload.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns true if this payload has the same identity as another.
    ///
    /// Identity is determined by asset path and prim path only;
    /// layer offset is ignored.
    pub fn identity_equal(&self, other: &Payload) -> bool {
        self.asset_path == other.asset_path && self.prim_path == other.prim_path
    }
}

impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.asset_path == other.asset_path
            && self.prim_path == other.prim_path
            && self.layer_offset == other.layer_offset
    }
}

impl Eq for Payload {}

impl PartialOrd for Payload {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Payload {
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

impl Hash for Payload {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset_path.hash(state);
        self.prim_path.hash(state);
        self.layer_offset.hash(state);
    }
}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Payload")
            .field("asset_path", &self.asset_path)
            .field("prim_path", &self.prim_path)
            .field("layer_offset", &self.layer_offset)
            .finish()
    }
}

impl fmt::Display for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_internal() {
            write!(f, "Payload({})", self.prim_path)
        } else if self.prim_path.is_empty() {
            write!(f, "Payload({})", self.asset_path)
        } else {
            write!(f, "Payload({} {})", self.asset_path, self.prim_path)
        }
    }
}

/// A vector of payloads.
pub type PayloadVector = Vec<Payload>;

/// Finds the index of a payload with the same identity as the given payload.
///
/// Identity is determined by asset path and prim path only.
/// Returns -1 if not found.
///
/// # Arguments
///
/// * `payloads` - The vector of payloads to search
/// * `payload_id` - The payload to find (by identity)
pub fn find_payload_by_identity(payloads: &PayloadVector, payload_id: &Payload) -> i32 {
    for (i, p) in payloads.iter().enumerate() {
        if p.identity_equal(payload_id) {
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
        let payload = Payload::new("model.usd", "/Model");
        assert_eq!(payload.asset_path(), "model.usd");
        assert_eq!(payload.prim_path().as_str(), "/Model");
    }

    #[test]
    fn test_default() {
        let payload = Payload::default();
        assert!(payload.asset_path().is_empty());
        assert!(payload.prim_path().is_empty());
        assert!(payload.is_internal());
    }

    #[test]
    fn test_internal() {
        let payload = Payload::internal("/SharedPrim");
        assert!(payload.is_internal());
        assert_eq!(payload.prim_path().as_str(), "/SharedPrim");
    }

    #[test]
    fn test_to_default_prim() {
        let payload = Payload::to_default_prim("model.usd");
        assert!(!payload.is_internal());
        assert!(payload.prim_path().is_empty());
    }

    #[test]
    fn test_with_layer_offset() {
        let payload =
            Payload::with_layer_offset("model.usd", "/Model", LayerOffset::new(10.0, 2.0));

        assert_eq!(payload.layer_offset().offset(), 10.0);
        assert_eq!(payload.layer_offset().scale(), 2.0);
    }

    #[test]
    fn test_setters() {
        let mut payload = Payload::default();
        payload.set_asset_path("model.usd");
        payload.set_prim_path(Path::from_string("/Model").unwrap());
        payload.set_layer_offset(LayerOffset::new(5.0, 1.0));

        assert_eq!(payload.asset_path(), "model.usd");
        assert_eq!(payload.prim_path().as_str(), "/Model");
        assert_eq!(payload.layer_offset().offset(), 5.0);
    }

    #[test]
    fn test_equality() {
        let p1 = Payload::new("model.usd", "/Model");
        let p2 = Payload::new("model.usd", "/Model");
        let p3 = Payload::new("other.usd", "/Model");

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_identity_equal() {
        let p1 = Payload::with_layer_offset("model.usd", "/Model", LayerOffset::new(10.0, 1.0));
        let p2 = Payload::with_layer_offset("model.usd", "/Model", LayerOffset::new(20.0, 2.0));

        // Different layer offsets but same identity
        assert!(p1.identity_equal(&p2));
        assert_ne!(p1, p2); // But not fully equal
    }

    #[test]
    fn test_ordering() {
        let p1 = Payload::new("a.usd", "/Model");
        let p2 = Payload::new("b.usd", "/Model");

        assert!(p1 < p2);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let p1 = Payload::new("model.usd", "/Model");
        let p2 = Payload::new("model.usd", "/Model");
        let p3 = Payload::new("other.usd", "/Model");

        let mut set = HashSet::new();
        set.insert(p1.clone());
        assert!(set.contains(&p2));
        assert!(!set.contains(&p3));
    }

    #[test]
    fn test_display() {
        let external = Payload::new("model.usd", "/Model");
        assert!(format!("{}", external).contains("model.usd"));
        assert!(format!("{}", external).contains("/Model"));

        let internal = Payload::internal("/SharedPrim");
        assert!(format!("{}", internal).contains("/SharedPrim"));

        let default_prim = Payload::to_default_prim("model.usd");
        assert!(format!("{}", default_prim).contains("model.usd"));
    }

    #[test]
    fn test_find_payload_by_identity() {
        let payloads = vec![
            Payload::new("a.usd", "/A"),
            Payload::new("b.usd", "/B"),
            Payload::new("c.usd", "/C"),
        ];

        let found = find_payload_by_identity(
            &payloads,
            &Payload::with_layer_offset("b.usd", "/B", LayerOffset::new(10.0, 1.0)),
        );
        assert_eq!(found, 1);

        let not_found = find_payload_by_identity(&payloads, &Payload::new("d.usd", "/D"));
        assert_eq!(not_found, -1);
    }
}
