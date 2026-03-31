//! Layer stack identifier.
//!
//! Identifies a layer stack by its root layer, session layer, and
//! resolver context.
//!
//! # Examples
//!
//! ```
//! use usd_pcp::LayerStackIdentifier;
//!
//! let id = LayerStackIdentifier::new("root.usda");
//! assert!(id.is_valid());
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};

use usd_ar::ResolverContext;
use usd_sdf::AssetPath;

/// Identifies a layer stack by its root layer, session layer, and context.
///
/// Layer stacks are identified by:
/// - A root layer (required)
/// - An optional session layer
/// - An optional resolver context for asset path resolution
///
/// # Examples
///
/// ```
/// use usd_pcp::LayerStackIdentifier;
///
/// // Simple identifier with just root layer
/// let id = LayerStackIdentifier::new("root.usda");
///
/// // With session layer
/// let id_with_session = LayerStackIdentifier::with_session(
///     "root.usda",
///     Some("session.usda")
/// );
/// ```
#[derive(Clone, Debug)]
pub struct LayerStackIdentifier {
    /// The root layer path.
    pub root_layer: AssetPath,

    /// The session layer path (optional).
    pub session_layer: Option<AssetPath>,

    /// The path resolver context (optional).
    pub resolver_context: Option<ResolverContext>,

    /// Cached hash value.
    hash: u64,
}

impl LayerStackIdentifier {
    /// Creates a new layer stack identifier with the given root layer.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::LayerStackIdentifier;
    ///
    /// let id = LayerStackIdentifier::new("root.usda");
    /// assert_eq!(id.root_layer.get_authored_path(), "root.usda");
    /// ```
    #[must_use]
    pub fn new(root_layer: impl Into<AssetPath>) -> Self {
        Self::with_parts(root_layer.into(), None, None)
    }

    /// Creates a layer stack identifier with root and session layers.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::LayerStackIdentifier;
    ///
    /// let id = LayerStackIdentifier::with_session("root.usda", Some("session.usda"));
    /// assert!(id.session_layer.is_some());
    /// ```
    #[must_use]
    pub fn with_session(
        root_layer: impl Into<AssetPath>,
        session_layer: Option<impl Into<AssetPath>>,
    ) -> Self {
        Self::with_parts(root_layer.into(), session_layer.map(Into::into), None)
    }

    /// Creates a layer stack identifier with all components.
    #[must_use]
    pub fn with_parts(
        root_layer: AssetPath,
        session_layer: Option<AssetPath>,
        resolver_context: Option<ResolverContext>,
    ) -> Self {
        let hash = Self::compute_hash(&root_layer, &session_layer, &resolver_context);
        Self {
            root_layer,
            session_layer,
            resolver_context,
            hash,
        }
    }

    /// Returns true if this identifier is valid (has a non-empty root layer).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::LayerStackIdentifier;
    ///
    /// let valid = LayerStackIdentifier::new("root.usda");
    /// assert!(valid.is_valid());
    ///
    /// let invalid = LayerStackIdentifier::new("");
    /// assert!(!invalid.is_valid());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.root_layer.get_authored_path().is_empty()
    }

    /// Returns the hash value.
    #[inline]
    #[must_use]
    pub fn get_hash(&self) -> u64 {
        self.hash
    }

    fn compute_hash(
        root_layer: &AssetPath,
        session_layer: &Option<AssetPath>,
        resolver_context: &Option<ResolverContext>,
    ) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        root_layer.get_authored_path().hash(&mut hasher);
        if let Some(session) = session_layer {
            session.get_authored_path().hash(&mut hasher);
        }
        if let Some(ctx) = resolver_context {
            ctx.hash(&mut hasher);
        }
        hasher.finish()
    }
}

impl Default for LayerStackIdentifier {
    fn default() -> Self {
        Self::new("")
    }
}

impl PartialEq for LayerStackIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.root_layer == other.root_layer
            && self.session_layer == other.session_layer
            && self.resolver_context == other.resolver_context
    }
}

impl Eq for LayerStackIdentifier {}

impl Hash for LayerStackIdentifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl PartialOrd for LayerStackIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LayerStackIdentifier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self
            .root_layer
            .get_authored_path()
            .cmp(other.root_layer.get_authored_path())
        {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match (&self.session_layer, &other.session_layer) {
            (Some(a), Some(b)) => match a.get_authored_path().cmp(b.get_authored_path()) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            },
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (None, None) => {}
        }
        std::cmp::Ordering::Equal
    }
}

impl fmt::Display for LayerStackIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root_layer)?;
        if let Some(session) = &self.session_layer {
            write!(f, " + {}", session)?;
        }
        Ok(())
    }
}

/// Hash functor for LayerStackIdentifier.
pub struct LayerStackIdentifierHash;

impl LayerStackIdentifierHash {
    /// Computes hash for a LayerStackIdentifier.
    #[inline]
    #[must_use]
    pub fn hash(id: &LayerStackIdentifier) -> u64 {
        id.get_hash()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let id = LayerStackIdentifier::new("root.usda");
        assert_eq!(id.root_layer.get_authored_path(), "root.usda");
        assert!(id.session_layer.is_none());
        assert!(id.resolver_context.is_none());
        assert!(id.is_valid());
    }

    #[test]
    fn test_with_session() {
        let id = LayerStackIdentifier::with_session("root.usda", Some("session.usda"));
        assert_eq!(id.root_layer.get_authored_path(), "root.usda");
        assert_eq!(
            id.session_layer.as_ref().map(|p| p.get_authored_path()),
            Some("session.usda")
        );
        assert!(id.is_valid());
    }

    #[test]
    fn test_is_valid() {
        let valid = LayerStackIdentifier::new("root.usda");
        assert!(valid.is_valid());

        let invalid = LayerStackIdentifier::new("");
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_equality() {
        let id1 = LayerStackIdentifier::new("root.usda");
        let id2 = LayerStackIdentifier::new("root.usda");
        let id3 = LayerStackIdentifier::new("other.usda");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_hash() {
        let id1 = LayerStackIdentifier::new("root.usda");
        let id2 = LayerStackIdentifier::new("root.usda");

        assert_eq!(id1.get_hash(), id2.get_hash());
    }

    #[test]
    fn test_ordering() {
        let id_a = LayerStackIdentifier::new("a.usda");
        let id_b = LayerStackIdentifier::new("b.usda");

        assert!(id_a < id_b);
        assert!(id_b > id_a);
    }

    #[test]
    fn test_display() {
        let id = LayerStackIdentifier::new("root.usda");
        assert_eq!(format!("{}", id), "root.usda");

        let id_with_session = LayerStackIdentifier::with_session("root.usda", Some("session.usda"));
        assert_eq!(format!("{}", id_with_session), "root.usda + session.usda");
    }

    #[test]
    fn test_default() {
        let id = LayerStackIdentifier::default();
        assert!(!id.is_valid());
    }
}
