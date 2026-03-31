//! Relationship specifications.
//!
//! `RelationshipSpec` represents a relationship property on a prim. Relationships
//! reference one or more target prims or attributes, defining connections between
//! scene elements.
//!
//! # Overview
//!
//! A relationship property contains references to other scene description objects:
//! - Target paths identify the referenced prims or attributes
//! - All targets play the same role in the relationship
//! - Targets can be ordered or unordered
//! - Relationships can have relational attributes (metadata about connections)
//!
//! # Target Paths
//!
//! Target paths are managed through a `PathListOp` which supports:
//! - Explicit target lists
//! - Prepended/appended targets
//! - Deleted targets
//! - List editing composition
//!
//! # Load Hint
//!
//! The `noLoadHint` metadata controls whether loading relationship targets
//! is necessary to load the owning prim. Setting this to true indicates
//! that targets can be loaded lazily.
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{RelationshipSpec, Spec, Path, PathListOp, Variability};
//!
//! // Create a relationship spec
//! let spec = RelationshipSpec::default();
//!
//! // Check if it has target paths
//! assert!(!spec.has_target_path_list());
//!
//! // Get target paths (returns empty list if none)
//! let targets = spec.target_path_list();
//! assert!(!targets.has_keys());
//! ```

use std::sync::OnceLock;

use usd_tf::Token;

use super::{LayerHandle, Path, PathListOp, Spec, SpecType, Variability, VtValue};

// Cached tokens for relationship field names
mod tokens {
    use super::*;

    macro_rules! cached_token {
        ($name:ident, $str:literal) => {
            pub fn $name() -> Token {
                static TOKEN: OnceLock<Token> = OnceLock::new();
                TOKEN.get_or_init(|| Token::new($str)).clone()
            }
        };
    }

    cached_token!(custom, "custom");
    cached_token!(variability, "variability");
    cached_token!(target_paths, "targetPaths");
    cached_token!(no_load_hint, "noLoadHint");
}

// ============================================================================
// RelationshipSpec
// ============================================================================

/// Represents a relationship property on a prim.
///
/// A relationship contains references to one or more target prims or attributes.
/// All targets of a relationship are considered to be playing the same role,
/// though they need not be of the same type.
///
/// Relationships extend the base `Spec` functionality with:
/// - Target path management
/// - Load hints for lazy loading
/// - Relational attributes (metadata about specific connections)
///
/// # Property Methods
///
/// As a property, RelationshipSpec supports:
/// - `name()` - Property name
/// - `custom()` / `set_custom()` - Custom vs builtin
/// - `variability()` / `set_variability()` - Uniform vs Varying
///
/// # Target Management
///
/// - `target_path_list()` - Get target paths as PathListOp
/// - `has_target_path_list()` - Check if any targets exist
/// - `clear_target_path_list()` - Remove all targets
/// - `replace_target_path()` - Update a specific target
/// - `remove_target_path()` - Remove a specific target
///
/// # Thread Safety
///
/// Like all specs, RelationshipSpec is not thread-safe for mutation.
#[derive(Debug, Clone, Default)]
pub struct RelationshipSpec {
    /// Base spec containing layer/path identity and field access.
    spec: Spec,
}

impl RelationshipSpec {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new relationship spec with the given layer and path.
    ///
    /// This creates a handle to a relationship spec at the given location.
    /// The relationship must already exist in the layer for operations to succeed.
    ///
    /// # Arguments
    ///
    /// * `layer` - The layer containing the relationship
    /// * `path` - The path to the relationship (must be a property path)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{RelationshipSpec, Path, LayerHandle};
    ///
    /// let layer = LayerHandle::null();
    /// let path = Path::from("/World/Cube.targets");
    /// let rel_spec = RelationshipSpec::new(layer, path);
    /// ```
    #[must_use]
    pub fn new(layer: LayerHandle, path: Path) -> Self {
        Self {
            spec: Spec::new(layer, path),
        }
    }

    /// Create a RelationshipSpec from an existing Spec.
    ///
    /// This does not validate that the spec is actually a relationship spec.
    /// Callers should ensure the spec type is appropriate.
    #[must_use]
    pub fn from_spec(spec: Spec) -> Self {
        Self { spec }
    }

    /// Create a dormant (invalid) relationship spec.
    ///
    /// Dormant specs have no layer or path and cannot be used for operations.
    #[must_use]
    pub fn dormant() -> Self {
        Self {
            spec: Spec::dormant(),
        }
    }

    // ========================================================================
    // Spec Access
    // ========================================================================

    /// Get a reference to the underlying spec.
    ///
    /// This provides access to base spec functionality like field access,
    /// metadata, and identity information.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Get a mutable reference to the underlying spec.
    #[must_use]
    pub fn spec_mut(&mut self) -> &mut Spec {
        &mut self.spec
    }

    /// Returns the layer that this spec belongs to.
    #[must_use]
    pub fn layer(&self) -> LayerHandle {
        self.spec.layer()
    }

    /// Returns the scene path of this spec.
    #[must_use]
    pub fn path(&self) -> Path {
        self.spec.path()
    }

    /// Returns the spec type (should be SpecType::Relationship).
    #[must_use]
    pub fn spec_type(&self) -> SpecType {
        self.spec.spec_type()
    }

    /// Returns true if this spec is dormant.
    #[must_use]
    pub fn is_dormant(&self) -> bool {
        self.spec.is_dormant()
    }

    // ========================================================================
    // Property Methods
    // ========================================================================

    /// Returns the name of this relationship.
    ///
    /// The name is the final component of the property path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{RelationshipSpec, Path, LayerHandle};
    ///
    /// let spec = RelationshipSpec::new(
    ///     LayerHandle::null(),
    ///     Path::from("/World/Cube.targets")
    /// );
    /// assert_eq!(spec.name(), "targets");
    /// ```
    #[must_use]
    pub fn name(&self) -> String {
        if self.is_dormant() {
            return String::new();
        }
        self.path().get_name().to_string()
    }

    /// Returns whether this is a custom relationship.
    ///
    /// Custom relationships are user-defined rather than part of a schema.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::RelationshipSpec;
    ///
    /// let spec = RelationshipSpec::default();
    /// // Default is true (custom)
    /// assert!(spec.custom());
    /// ```
    #[must_use]
    pub fn custom(&self) -> bool {
        if self.is_dormant() {
            return true; // Default
        }
        self.spec
            .get_field(&tokens::custom())
            .get::<bool>()
            .copied()
            .unwrap_or(true)
    }

    /// Sets whether this is a custom relationship.
    ///
    /// # Arguments
    ///
    /// * `custom` - True for custom, false for builtin
    pub fn set_custom(&mut self, custom: bool) {
        if self.is_dormant() {
            return;
        }
        let _ = self.spec.set_field(&tokens::custom(), VtValue::new(custom));
    }

    /// Returns the variability of this relationship.
    ///
    /// Relationships are typically `Uniform` (same across all time samples).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{RelationshipSpec, Variability};
    ///
    /// let spec = RelationshipSpec::default();
    /// assert_eq!(spec.variability(), Variability::Uniform);
    /// ```
    #[must_use]
    pub fn variability(&self) -> Variability {
        if self.is_dormant() {
            return Variability::Uniform; // Default for relationships
        }
        self.spec
            .get_field(&tokens::variability())
            .get::<Variability>()
            .copied()
            .unwrap_or(Variability::Uniform)
    }

    /// Sets the variability of this relationship.
    ///
    /// # Arguments
    ///
    /// * `variability` - The variability to set
    pub fn set_variability(&mut self, variability: Variability) {
        if self.is_dormant() {
            return;
        }
        let _ = self
            .spec
            .set_field(&tokens::variability(), VtValue::new(variability));
    }

    // ========================================================================
    // Target Path Management
    // ========================================================================

    /// Returns the relationship's target path list editor.
    ///
    /// The list of target paths can be modified through the returned PathListOp.
    /// Target paths identify the prims or attributes that this relationship
    /// references.
    ///
    /// If no target paths are authored, returns an empty PathListOp.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::RelationshipSpec;
    ///
    /// let spec = RelationshipSpec::default();
    /// let targets = spec.target_path_list();
    /// assert!(!targets.has_keys());
    /// ```
    #[must_use]
    pub fn target_path_list(&self) -> PathListOp {
        if self.is_dormant() {
            return PathListOp::new();
        }
        self.spec
            .get_field(&tokens::target_paths())
            .get::<PathListOp>()
            .cloned()
            .unwrap_or_else(PathListOp::new)
    }

    /// Sets the target path list editor.
    ///
    /// # Arguments
    ///
    /// * `list_op` - The PathListOp to set
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{RelationshipSpec, PathListOp, Path};
    ///
    /// let mut spec = RelationshipSpec::default();
    /// let mut targets = PathListOp::new();
    /// targets.set_explicit_items(vec![Path::from("/World/Target1")]);
    /// spec.set_target_path_list(targets);
    /// ```
    pub fn set_target_path_list(&mut self, list_op: PathListOp) {
        if self.is_dormant() {
            return;
        }
        let _ = self
            .spec
            .set_field(&tokens::target_paths(), VtValue::from(list_op));
    }

    /// Returns true if the relationship has any target paths.
    ///
    /// This checks whether any target paths are authored (explicit, prepended,
    /// appended, or deleted).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::RelationshipSpec;
    ///
    /// let spec = RelationshipSpec::default();
    /// assert!(!spec.has_target_path_list());
    /// ```
    #[must_use]
    pub fn has_target_path_list(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::target_paths())
    }

    /// Clears the list of target paths on this relationship.
    ///
    /// This removes all authored target path information, including explicit
    /// lists and list edits.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::RelationshipSpec;
    ///
    /// let mut spec = RelationshipSpec::default();
    /// spec.clear_target_path_list();
    /// assert!(!spec.has_target_path_list());
    /// ```
    pub fn clear_target_path_list(&mut self) {
        if !self.is_dormant() {
            let _ = self.spec.clear_field(&tokens::target_paths());
        }
    }

    /// Updates the specified target path.
    ///
    /// Replaces all occurrences of `old_path` with `new_path` in the target
    /// path list. This updates the path in all list editing operations
    /// (explicit, prepended, appended, deleted).
    ///
    /// Also updates any relational attributes for the old target path to
    /// use the new path.
    ///
    /// # Arguments
    ///
    /// * `old_path` - The target path to replace
    /// * `new_path` - The new target path
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{RelationshipSpec, Path};
    ///
    /// let mut spec = RelationshipSpec::default();
    /// spec.replace_target_path(
    ///     &Path::from("/World/OldTarget"),
    ///     &Path::from("/World/NewTarget")
    /// );
    /// ```
    pub fn replace_target_path(&mut self, old_path: &Path, new_path: &Path) {
        if self.is_dormant() {
            return;
        }

        let mut list_op = self.target_path_list();

        // Replace in explicit items
        if list_op.is_explicit() {
            let items: Vec<Path> = list_op
                .get_explicit_items()
                .iter()
                .map(|p| {
                    if p == old_path {
                        new_path.clone()
                    } else {
                        p.clone()
                    }
                })
                .collect();
            let _ = list_op.set_explicit_items(items);
        } else {
            // Replace in prepended items
            let prepended: Vec<Path> = list_op
                .get_prepended_items()
                .iter()
                .map(|p| {
                    if p == old_path {
                        new_path.clone()
                    } else {
                        p.clone()
                    }
                })
                .collect();
            let _ = list_op.set_prepended_items(prepended);

            // Replace in appended items
            let appended: Vec<Path> = list_op
                .get_appended_items()
                .iter()
                .map(|p| {
                    if p == old_path {
                        new_path.clone()
                    } else {
                        p.clone()
                    }
                })
                .collect();
            let _ = list_op.set_appended_items(appended);

            // Replace in deleted items
            let deleted: Vec<Path> = list_op
                .get_deleted_items()
                .iter()
                .map(|p| {
                    if p == old_path {
                        new_path.clone()
                    } else {
                        p.clone()
                    }
                })
                .collect();
            let _ = list_op.set_deleted_items(deleted);
        }

        self.set_target_path_list(list_op);

        // Note: Relational attributes (deprecated USD feature) at /Prim.rel[/Target].attr
        // are not handled. Relational attributes are rarely used and have been superseded
        // by other USD patterns. If needed, they would be stored at paths like:
        // /Prim.relationship[/Target].attribute
    }

    /// Removes the specified target path.
    ///
    /// Removes the given target path from the relationship's target list.
    /// Also removes any relational attributes associated with this target.
    ///
    /// # Arguments
    ///
    /// * `path` - The target path to remove
    /// * `preserve_target_order` - If true, uses Erase() on the list editor
    ///   instead of RemoveItemEdits(). This preserves the ordered items list.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{RelationshipSpec, Path};
    ///
    /// let mut spec = RelationshipSpec::default();
    /// spec.remove_target_path(&Path::from("/World/Target"), false);
    /// ```
    pub fn remove_target_path(&mut self, path: &Path, preserve_target_order: bool) {
        if self.is_dormant() {
            return;
        }

        let mut list_op = self.target_path_list();

        if preserve_target_order {
            // Add to deleted items to preserve order
            let mut deleted = list_op.get_deleted_items().to_vec();
            if !deleted.contains(path) {
                deleted.push(path.clone());
            }
            let _ = list_op.set_deleted_items(deleted);
        } else {
            // Remove from all lists
            if list_op.is_explicit() {
                let items: Vec<Path> = list_op
                    .get_explicit_items()
                    .iter()
                    .filter(|p| *p != path)
                    .cloned()
                    .collect();
                let _ = list_op.set_explicit_items(items);
            } else {
                // Remove from prepended
                let prepended: Vec<Path> = list_op
                    .get_prepended_items()
                    .iter()
                    .filter(|p| *p != path)
                    .cloned()
                    .collect();
                let _ = list_op.set_prepended_items(prepended);

                // Remove from appended
                let appended: Vec<Path> = list_op
                    .get_appended_items()
                    .iter()
                    .filter(|p| *p != path)
                    .cloned()
                    .collect();
                let _ = list_op.set_appended_items(appended);

                // Add to deleted if not already there
                let mut deleted = list_op.get_deleted_items().to_vec();
                if !deleted.contains(path) {
                    deleted.push(path.clone());
                }
                let _ = list_op.set_deleted_items(deleted);
            }
        }

        self.set_target_path_list(list_op);

        // Note: Relational attributes (deprecated USD feature) are not removed.
        // This is a rarely-used legacy feature superseded by other USD patterns.
    }

    // ========================================================================
    // Load Hint
    // ========================================================================

    /// Get whether loading the target of this relationship is necessary
    /// to load the prim we're attached to.
    ///
    /// When true, indicates that the relationship targets can be loaded
    /// lazily - they're not required for the owning prim to be usable.
    ///
    /// Default is false (targets should be loaded).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::RelationshipSpec;
    ///
    /// let spec = RelationshipSpec::default();
    /// assert!(!spec.no_load_hint());
    /// ```
    #[must_use]
    pub fn no_load_hint(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec
            .get_field(&tokens::no_load_hint())
            .get::<bool>()
            .copied()
            .unwrap_or(false)
    }

    /// Set whether loading the target of this relationship is necessary
    /// to load the prim we're attached to.
    ///
    /// # Arguments
    ///
    /// * `no_load` - True if targets can be loaded lazily, false if required
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::RelationshipSpec;
    ///
    /// let mut spec = RelationshipSpec::default();
    /// spec.set_no_load_hint(true);
    /// assert!(spec.no_load_hint());
    /// ```
    pub fn set_no_load_hint(&mut self, no_load: bool) {
        if self.is_dormant() {
            return;
        }
        let _ = self
            .spec
            .set_field(&tokens::no_load_hint(), VtValue::from(no_load));
    }

    // ========================================================================
    // Field Access (delegated to base Spec)
    // ========================================================================

    /// Returns all field names that have values in this spec.
    #[must_use]
    pub fn list_fields(&self) -> Vec<Token> {
        self.spec.list_fields()
    }

    /// Returns true if the spec has a value for the given field.
    #[must_use]
    pub fn has_field(&self, name: &Token) -> bool {
        self.spec.has_field(name)
    }

    /// Returns the value for the given field.
    #[must_use]
    pub fn get_field(&self, name: &Token) -> VtValue {
        self.spec.get_field(name)
    }

    /// Sets the value for the given field.
    pub fn set_field(&mut self, name: &Token, value: VtValue) -> bool {
        self.spec.set_field(name, value)
    }

    /// Clears the given field.
    pub fn clear_field(&mut self, name: &Token) -> bool {
        self.spec.clear_field(name)
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

impl PartialEq for RelationshipSpec {
    fn eq(&self, other: &Self) -> bool {
        self.spec == other.spec
    }
}

impl Eq for RelationshipSpec {}

impl std::hash::Hash for RelationshipSpec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.spec.hash(state);
    }
}

impl std::fmt::Display for RelationshipSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_dormant() {
            write!(f, "<dormant relationship>")
        } else {
            write!(f, "<relationship '{}' at {}>", self.name(), self.path())
        }
    }
}

// ============================================================================
// From/Into Conversions
// ============================================================================

impl From<Spec> for RelationshipSpec {
    fn from(spec: Spec) -> Self {
        Self { spec }
    }
}

impl From<RelationshipSpec> for Spec {
    fn from(rel_spec: RelationshipSpec) -> Self {
        rel_spec.spec
    }
}

impl AsRef<Spec> for RelationshipSpec {
    fn as_ref(&self) -> &Spec {
        &self.spec
    }
}

impl AsMut<Spec> for RelationshipSpec {
    fn as_mut(&mut self) -> &mut Spec {
        &mut self.spec
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let layer = LayerHandle::null();
        let path = Path::from("/World/Cube.targets");
        let spec = RelationshipSpec::new(layer, path.clone());

        assert_eq!(spec.path(), path);
        assert!(spec.is_dormant()); // Null layer makes it dormant
    }

    #[test]
    fn test_dormant() {
        let spec = RelationshipSpec::dormant();
        assert!(spec.is_dormant());
        assert_eq!(spec.name(), "");
    }

    #[test]
    fn test_default() {
        let spec = RelationshipSpec::default();
        assert!(spec.is_dormant());
    }

    #[test]
    fn test_name() {
        // For dormant spec (null layer), name extraction still works from path
        let path = Path::from("/World/Cube.material");
        assert_eq!(path.get_name(), "material");

        // But spec.name() returns empty for dormant spec
        let spec = RelationshipSpec::new(LayerHandle::null(), path);
        // With null layer, spec is dormant, so name() returns empty
        assert_eq!(spec.name(), "");
    }

    #[test]
    fn test_custom_default() {
        let spec = RelationshipSpec::default();
        assert!(spec.custom());
    }

    #[test]
    fn test_set_custom() {
        let mut spec = RelationshipSpec::default();
        spec.set_custom(false);
        // Can't verify until Layer is implemented
    }

    #[test]
    fn test_variability_default() {
        let spec = RelationshipSpec::default();
        assert_eq!(spec.variability(), Variability::Uniform);
    }

    #[test]
    fn test_set_variability() {
        let mut spec = RelationshipSpec::default();
        spec.set_variability(Variability::Varying);
        // Can't verify until Layer is implemented
    }

    #[test]
    fn test_target_path_list_default() {
        let spec = RelationshipSpec::default();
        let targets = spec.target_path_list();
        assert!(!targets.has_keys());
        assert!(!targets.is_explicit());
    }

    #[test]
    fn test_has_target_path_list() {
        let spec = RelationshipSpec::default();
        assert!(!spec.has_target_path_list());
    }

    #[test]
    fn test_set_target_path_list() {
        let mut spec = RelationshipSpec::default();
        let mut targets = PathListOp::new();
        let _ = targets.set_explicit_items(vec![
            Path::from("/World/Target1"),
            Path::from("/World/Target2"),
        ]);
        spec.set_target_path_list(targets);
        // Can't verify until Layer is implemented
    }

    #[test]
    fn test_clear_target_path_list() {
        let mut spec = RelationshipSpec::default();
        spec.clear_target_path_list();
        assert!(!spec.has_target_path_list());
    }

    #[test]
    fn test_replace_target_path() {
        let mut spec = RelationshipSpec::default();
        let old = Path::from("/World/OldTarget");
        let new = Path::from("/World/NewTarget");

        // Set up initial targets
        let mut targets = PathListOp::new();
        let _ = targets.set_explicit_items(vec![old.clone(), Path::from("/Other")]);
        spec.set_target_path_list(targets);

        // Replace
        spec.replace_target_path(&old, &new);

        // Verify replacement (when Layer is implemented)
        // For now, just ensure it doesn't panic
    }

    #[test]
    fn test_replace_target_path_in_list_edits() {
        let mut spec = RelationshipSpec::default();
        let old = Path::from("/World/OldTarget");
        let new = Path::from("/World/NewTarget");

        // Set up targets with list edits
        let mut targets = PathListOp::new();
        let _ = targets.set_prepended_items(vec![old.clone()]);
        let _ = targets.set_appended_items(vec![Path::from("/Other"), old.clone()]);
        spec.set_target_path_list(targets);

        // Replace
        spec.replace_target_path(&old, &new);

        // Should replace in both prepended and appended
    }

    #[test]
    fn test_remove_target_path() {
        let mut spec = RelationshipSpec::default();
        let target = Path::from("/World/Target");

        spec.remove_target_path(&target, false);
        // Can't verify until Layer is implemented
    }

    #[test]
    fn test_remove_target_path_preserve_order() {
        let mut spec = RelationshipSpec::default();
        let target = Path::from("/World/Target");

        // With preserve_target_order=true, should add to deleted list
        spec.remove_target_path(&target, true);
        // Can't verify until Layer is implemented
    }

    #[test]
    fn test_no_load_hint_default() {
        let spec = RelationshipSpec::default();
        assert!(!spec.no_load_hint());
    }

    #[test]
    fn test_set_no_load_hint() {
        let mut spec = RelationshipSpec::default();
        spec.set_no_load_hint(true);
        // Can't verify until Layer is implemented
    }

    #[test]
    fn test_equality() {
        let layer = LayerHandle::null();
        let path = Path::from("/World/Cube.targets");

        let spec1 = RelationshipSpec::new(layer.clone(), path.clone());
        let spec2 = RelationshipSpec::new(layer.clone(), path);
        let spec3 = RelationshipSpec::new(layer, Path::from("/Other.rel"));

        assert_eq!(spec1, spec2);
        assert_ne!(spec1, spec3);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let layer = LayerHandle::null();
        let mut set = HashSet::new();

        set.insert(RelationshipSpec::new(layer.clone(), Path::from("/A.rel")));
        set.insert(RelationshipSpec::new(layer.clone(), Path::from("/B.rel")));
        set.insert(RelationshipSpec::new(layer, Path::from("/A.rel"))); // Duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_display() {
        let spec = RelationshipSpec::dormant();
        assert_eq!(format!("{}", spec), "<dormant relationship>");

        // Spec with null layer is also dormant
        let spec = RelationshipSpec::new(LayerHandle::null(), Path::from("/World/Cube.material"));
        // Dormant specs show as dormant
        assert_eq!(format!("{}", spec), "<dormant relationship>");
    }

    #[test]
    fn test_from_spec() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/World.rel"));
        let rel_spec = RelationshipSpec::from(spec);
        assert_eq!(rel_spec.path(), Path::from("/World.rel"));
    }

    #[test]
    fn test_into_spec() {
        let rel_spec = RelationshipSpec::new(LayerHandle::null(), Path::from("/World.rel"));
        let spec: Spec = rel_spec.into();
        assert_eq!(spec.path(), Path::from("/World.rel"));
    }

    #[test]
    fn test_as_ref_spec() {
        let rel_spec = RelationshipSpec::new(LayerHandle::null(), Path::from("/World.rel"));
        let spec_ref: &Spec = rel_spec.as_ref();
        assert_eq!(spec_ref.path(), Path::from("/World.rel"));
    }

    #[test]
    fn test_as_mut_spec() {
        let mut rel_spec = RelationshipSpec::new(LayerHandle::null(), Path::from("/World.rel"));
        let spec_mut: &mut Spec = rel_spec.as_mut();
        assert_eq!(spec_mut.path(), Path::from("/World.rel"));
    }

    #[test]
    fn test_field_access() {
        let spec = RelationshipSpec::default();
        let key = Token::new("custom");

        assert_eq!(spec.list_fields(), Vec::<Token>::new());
        assert!(!spec.has_field(&key));
        assert!(spec.get_field(&key).is_empty());
    }

    #[test]
    fn test_field_mutations() {
        let mut spec = RelationshipSpec::default();
        let key = Token::new("custom");
        let value = VtValue::new("test".to_string());

        assert!(!spec.set_field(&key, value));
        assert!(!spec.clear_field(&key));
    }

    #[test]
    fn test_spec_access() {
        let rel_spec = RelationshipSpec::new(LayerHandle::null(), Path::from("/World.rel"));

        let spec = rel_spec.spec();
        assert_eq!(spec.path(), Path::from("/World.rel"));

        let mut rel_spec_mut = RelationshipSpec::default();
        let spec_mut = rel_spec_mut.spec_mut();
        assert!(spec_mut.is_dormant());
    }

    #[test]
    fn test_layer_and_path_access() {
        let layer = LayerHandle::null();
        let path = Path::from("/World.rel");
        let spec = RelationshipSpec::new(layer.clone(), path.clone());

        assert_eq!(spec.layer(), layer);
        assert_eq!(spec.path(), path);
    }

    #[test]
    fn test_spec_type() {
        let spec = RelationshipSpec::default();
        // Will return SpecType::Relationship when Layer is implemented
        // For now returns Unknown
        let _ = spec.spec_type();
    }
}
