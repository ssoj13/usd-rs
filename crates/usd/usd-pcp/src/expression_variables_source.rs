//! Expression variables source.
//!
//! Represents the layer stack associated with a set of expression variables.
//!
//! # Overview
//!
//! Expression variables are key-value pairs that can be used in asset path
//! expressions. The source of these variables is typically a layer stack,
//! identified by a [`LayerStackIdentifier`].
//!
//! A `None` identifier represents the root layer stack of a prim index.
//!
//! # Examples
//!
//! ```
//! use usd_pcp::{ExpressionVariablesSource, LayerStackIdentifier};
//!
//! // Create a source representing the root layer stack
//! let root_source = ExpressionVariablesSource::new();
//! assert!(root_source.is_root_layer_stack());
//!
//! // Create a source for a specific layer stack
//! let id = LayerStackIdentifier::new("model.usda");
//! let root_id = LayerStackIdentifier::new("root.usda");
//! let source = ExpressionVariablesSource::from_identifier(&id, &root_id);
//! ```

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::LayerStackIdentifier;

/// Represents the layer stack associated with a set of expression variables.
///
/// This is typically a simple [`LayerStackIdentifier`]. A `None` value for
/// the identifier represents the root layer stack of a prim index.
///
/// # Examples
///
/// ```
/// use usd_pcp::{ExpressionVariablesSource, LayerStackIdentifier};
///
/// let source = ExpressionVariablesSource::new();
/// assert!(source.is_root_layer_stack());
/// assert!(source.get_layer_stack_identifier().is_none());
/// ```
#[derive(Clone, Debug, Default)]
pub struct ExpressionVariablesSource {
    /// The identifier of the layer stack providing expression variables.
    /// None indicates the root layer stack.
    identifier: Option<Arc<LayerStackIdentifier>>,
}

impl ExpressionVariablesSource {
    /// Creates a source representing the root layer stack of a prim index.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::ExpressionVariablesSource;
    ///
    /// let source = ExpressionVariablesSource::new();
    /// assert!(source.is_root_layer_stack());
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self { identifier: None }
    }

    /// Creates a source representing the given layer stack.
    ///
    /// If `layer_stack_identifier` equals `root_layer_stack_identifier`,
    /// this is equivalent to the default constructor (root layer stack).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariablesSource, LayerStackIdentifier};
    ///
    /// let id = LayerStackIdentifier::new("model.usda");
    /// let root_id = LayerStackIdentifier::new("root.usda");
    /// let source = ExpressionVariablesSource::from_identifier(&id, &root_id);
    ///
    /// // Different from root, so has identifier
    /// assert!(!source.is_root_layer_stack());
    /// ```
    #[must_use]
    pub fn from_identifier(
        layer_stack_identifier: &LayerStackIdentifier,
        root_layer_stack_identifier: &LayerStackIdentifier,
    ) -> Self {
        if layer_stack_identifier == root_layer_stack_identifier {
            Self::new()
        } else {
            Self {
                identifier: Some(Arc::new(layer_stack_identifier.clone())),
            }
        }
    }

    /// Returns true if this object represents a prim index's root layer stack.
    ///
    /// If this returns true, [`get_layer_stack_identifier`](Self::get_layer_stack_identifier)
    /// will return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::ExpressionVariablesSource;
    ///
    /// let source = ExpressionVariablesSource::new();
    /// assert!(source.is_root_layer_stack());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_root_layer_stack(&self) -> bool {
        self.identifier.is_none()
    }

    /// Returns the identifier of the layer stack represented by this object.
    ///
    /// Returns `None` if this object represents the root layer stack
    /// (i.e., [`is_root_layer_stack`](Self::is_root_layer_stack) returns true).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariablesSource, LayerStackIdentifier};
    ///
    /// let source = ExpressionVariablesSource::new();
    /// assert!(source.get_layer_stack_identifier().is_none());
    ///
    /// let id = LayerStackIdentifier::new("model.usda");
    /// let root_id = LayerStackIdentifier::new("root.usda");
    /// let source2 = ExpressionVariablesSource::from_identifier(&id, &root_id);
    /// assert!(source2.get_layer_stack_identifier().is_some());
    /// ```
    #[inline]
    #[must_use]
    pub fn get_layer_stack_identifier(&self) -> Option<&LayerStackIdentifier> {
        self.identifier.as_deref()
    }

    /// Returns the identifier of the layer stack represented by this object.
    ///
    /// If this object represents the root layer stack, returns
    /// `root_layer_stack_identifier`. Otherwise returns the stored identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariablesSource, LayerStackIdentifier};
    ///
    /// let root_id = LayerStackIdentifier::new("root.usda");
    ///
    /// // Root source resolves to root identifier
    /// let source = ExpressionVariablesSource::new();
    /// let resolved = source.resolve_layer_stack_identifier(&root_id);
    /// assert_eq!(resolved, &root_id);
    ///
    /// // Non-root source resolves to its own identifier
    /// let model_id = LayerStackIdentifier::new("model.usda");
    /// let source2 = ExpressionVariablesSource::from_identifier(&model_id, &root_id);
    /// let resolved2 = source2.resolve_layer_stack_identifier(&root_id);
    /// assert_eq!(resolved2, &model_id);
    /// ```
    #[must_use]
    pub fn resolve_layer_stack_identifier<'a>(
        &'a self,
        root_layer_stack_identifier: &'a LayerStackIdentifier,
    ) -> &'a LayerStackIdentifier {
        self.identifier
            .as_deref()
            .unwrap_or(root_layer_stack_identifier)
    }

    /// Returns a hash value for this object.
    #[must_use]
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl PartialEq for ExpressionVariablesSource {
    fn eq(&self, other: &Self) -> bool {
        match (&self.identifier, &other.identifier) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ExpressionVariablesSource {}

impl PartialOrd for ExpressionVariablesSource {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExpressionVariablesSource {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.identifier, &other.identifier) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

impl Hash for ExpressionVariablesSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identifier.is_some().hash(state);
        if let Some(id) = &self.identifier {
            id.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let source = ExpressionVariablesSource::new();
        assert!(source.is_root_layer_stack());
        assert!(source.get_layer_stack_identifier().is_none());
    }

    #[test]
    fn test_default() {
        let source = ExpressionVariablesSource::default();
        assert!(source.is_root_layer_stack());
    }

    #[test]
    fn test_from_identifier_same_as_root() {
        let id = LayerStackIdentifier::new("root.usda");
        let source = ExpressionVariablesSource::from_identifier(&id, &id);
        assert!(source.is_root_layer_stack());
    }

    #[test]
    fn test_from_identifier_different_from_root() {
        let id = LayerStackIdentifier::new("model.usda");
        let root_id = LayerStackIdentifier::new("root.usda");
        let source = ExpressionVariablesSource::from_identifier(&id, &root_id);

        assert!(!source.is_root_layer_stack());
        assert!(source.get_layer_stack_identifier().is_some());
        assert_eq!(
            source
                .get_layer_stack_identifier()
                .unwrap()
                .root_layer
                .get_authored_path(),
            "model.usda"
        );
    }

    #[test]
    fn test_resolve_layer_stack_identifier_root() {
        let root_id = LayerStackIdentifier::new("root.usda");
        let source = ExpressionVariablesSource::new();

        let resolved = source.resolve_layer_stack_identifier(&root_id);
        assert_eq!(resolved, &root_id);
    }

    #[test]
    fn test_resolve_layer_stack_identifier_non_root() {
        let model_id = LayerStackIdentifier::new("model.usda");
        let root_id = LayerStackIdentifier::new("root.usda");
        let source = ExpressionVariablesSource::from_identifier(&model_id, &root_id);

        let resolved = source.resolve_layer_stack_identifier(&root_id);
        assert_eq!(resolved, &model_id);
    }

    #[test]
    fn test_equality() {
        let source1 = ExpressionVariablesSource::new();
        let source2 = ExpressionVariablesSource::new();
        assert_eq!(source1, source2);

        let id = LayerStackIdentifier::new("model.usda");
        let root_id = LayerStackIdentifier::new("root.usda");
        let source3 = ExpressionVariablesSource::from_identifier(&id, &root_id);
        let source4 = ExpressionVariablesSource::from_identifier(&id, &root_id);
        assert_eq!(source3, source4);

        assert_ne!(source1, source3);
    }

    #[test]
    fn test_ordering() {
        let root_source = ExpressionVariablesSource::new();

        let id_a = LayerStackIdentifier::new("a.usda");
        let id_b = LayerStackIdentifier::new("b.usda");
        let root_id = LayerStackIdentifier::new("root.usda");

        let source_a = ExpressionVariablesSource::from_identifier(&id_a, &root_id);
        let source_b = ExpressionVariablesSource::from_identifier(&id_b, &root_id);

        // Root is less than any non-root
        assert!(root_source < source_a);
        assert!(root_source < source_b);

        // Ordering follows identifier ordering
        assert!(source_a < source_b);
    }

    #[test]
    fn test_hash() {
        let source1 = ExpressionVariablesSource::new();
        let source2 = ExpressionVariablesSource::new();
        assert_eq!(source1.get_hash(), source2.get_hash());

        let id = LayerStackIdentifier::new("model.usda");
        let root_id = LayerStackIdentifier::new("root.usda");
        let source3 = ExpressionVariablesSource::from_identifier(&id, &root_id);
        let source4 = ExpressionVariablesSource::from_identifier(&id, &root_id);
        assert_eq!(source3.get_hash(), source4.get_hash());
    }

    #[test]
    fn test_clone() {
        let id = LayerStackIdentifier::new("model.usda");
        let root_id = LayerStackIdentifier::new("root.usda");
        let source = ExpressionVariablesSource::from_identifier(&id, &root_id);
        let cloned = source.clone();

        assert_eq!(source, cloned);
    }
}
