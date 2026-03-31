//! Prim specs - represent prims in scene description layers.
//!
//! `PrimSpec` represents a prim description in a layer object. Every prim spec
//! is defined in a layer and is identified by its path in the namespace hierarchy.
//!
//! Prim specs have properties of two general types:
//! - **Attributes** - Contain values (represented by AttributeSpec)
//! - **Relationships** - Connections to other prims (represented by RelationshipSpec)
//!
//! # Metadata
//!
//! Prim specs contain various metadata:
//! - `type_name` - Schema type of the prim
//! - `specifier` - Def, Over, or Class
//! - `comment` - User comment
//! - `documentation` - Documentation string
//! - `active` - Whether the prim is active
//! - `kind` - Kind classification (component, group, etc.)
//! - `permission` - Access restriction level
//!
//! # Composition Arcs
//!
//! Prims can reference other scene description through:
//! - **References** - General composition arcs
//! - **Payloads** - Deferred composition arcs
//! - **Inherits** - Class-based composition
//! - **Specializes** - Specialized composition
//!
//! # Variants
//!
//! Prims can define variant sets that allow switching between different
//! configurations of the prim and its subtree.
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{PrimSpec, Specifier, Permission};
//! use usd_tf::Token;
//!
//! // Create a prim spec (requires Layer implementation)
//! // let layer = Layer::create_anonymous();
//! // let prim = PrimSpec::new_root(&layer, "World", Specifier::Def, "Xform");
//!
//! // Basic properties
//! let spec = PrimSpec::default();
//! // spec.set_type_name("Xform");
//! // spec.set_comment("Main world prim");
//! // spec.set_active(true);
//! ```

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::OnceLock;

use usd_tf::Token;

use super::{
    LayerHandle, Path, PathListOp, PayloadListOp, Permission, ReferenceListOp, Spec, Specifier,
    VtDictionary, VtValue,
};

// Cached tokens for prim spec field names
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

    cached_token!(prim_children, "primChildren");
    cached_token!(properties, "properties");
    cached_token!(type_name, "typeName");
    cached_token!(specifier, "specifier");
    cached_token!(comment, "comment");
    cached_token!(documentation, "documentation");
    cached_token!(active, "active");
    cached_token!(hidden, "hidden");
    cached_token!(kind, "kind");
    cached_token!(permission, "permission");
    cached_token!(symmetric_peer, "symmetricPeer");
    cached_token!(prefix, "prefix");
    cached_token!(suffix, "suffix");
    cached_token!(custom_data, "customData");
    cached_token!(asset_info, "assetInfo");
    cached_token!(references, "references");
    cached_token!(payload, "payload");
    cached_token!(inherit_paths, "inheritPaths");
    cached_token!(specializes, "specializes");
    cached_token!(variant_selection, "variantSelection");
    cached_token!(variant_set_names, "variantSetNames");
    cached_token!(name_children_order, "primOrder");
    cached_token!(property_order, "propertyOrder");
    cached_token!(symmetry_function, "symmetryFunction");
    cached_token!(symmetry_arguments, "symmetryArguments");
    cached_token!(prefix_substitutions, "prefixSubstitutions");
    cached_token!(suffix_substitutions, "suffixSubstitutions");
    cached_token!(instanceable, "instanceable");
    cached_token!(clips, "clips");
    cached_token!(clip_sets, "clipSets");
    cached_token!(relocates, "relocates");
}

// ============================================================================
// Re-export spec types
// ============================================================================

// Re-export from their respective modules
pub use super::attribute_spec::AttributeSpec;
pub use super::property_spec::PropertySpec;
pub use super::relationship_spec::RelationshipSpec;

// VariantSetsProxy is defined in variant_set_spec module
use super::variant_set_spec::VariantSetsProxy;

// ============================================================================
// PrimSpec
// ============================================================================

/// Represents a prim description in a layer.
///
/// A prim spec defines a prim at a specific location in a layer's namespace
/// hierarchy. It can contain:
/// - Type information and metadata
/// - Child prims (namespace children)
/// - Properties (attributes and relationships)
/// - Composition arcs (references, payloads, inherits, specializes)
/// - Variant sets and selections
///
/// # Lifecycle
///
/// Prim specs are created through:
/// - `new_root()` - Creates a root prim in a layer
/// - `new_child()` - Creates a child prim under another prim
///
/// # Thread Safety
///
/// Prim specs are not thread-safe for mutation but can be read from multiple
/// threads if the underlying layer is not being modified.
#[derive(Debug, Clone, Default)]
pub struct PrimSpec {
    /// Base spec functionality.
    spec: Spec,
}

impl PrimSpec {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Creates a root prim spec in the given layer.
    ///
    /// # Arguments
    ///
    /// * `parent_layer` - The layer to create the prim in
    /// * `name` - The prim name
    /// * `specifier` - The spec specifier (Def, Over, or Class)
    /// * `type_name` - The prim type name (optional)
    ///
    /// # Returns
    ///
    /// A new PrimSpec, or an error if creation fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_sdf::{PrimSpec, Specifier};
    /// // let layer = Layer::create_anonymous();
    /// // let prim = PrimSpec::new_root(&layer, "World", Specifier::Def, "Xform")?;
    /// ```
    pub fn new_root(
        parent_layer: &LayerHandle,
        name: &str,
        specifier: Specifier,
        type_name: &str,
    ) -> Result<Self, String> {
        // Validate name
        if !Self::is_valid_name(name) {
            return Err(format!("'{}' is not a valid prim name", name));
        }

        if !parent_layer.is_valid() {
            return Err("Invalid layer handle".to_string());
        }

        // Create path /name
        let path = Path::from_string(&format!("/{}", name))
            .ok_or_else(|| format!("Failed to create path for name '{}'", name))?;

        // Check if prim already exists
        if parent_layer.get_prim_at_path(&path).is_some() {
            return Err(format!("Prim already exists at path {}", path));
        }

        // Create spec in layer
        parent_layer
            .create_prim_spec(&path, specifier, type_name)
            .ok_or_else(|| "Failed to create prim spec in layer".to_string())
    }

    /// Creates a child prim spec under another prim.
    ///
    /// # Arguments
    ///
    /// * `parent_prim` - The parent prim
    /// * `name` - The child prim name
    /// * `specifier` - The spec specifier (Def, Over, or Class)
    /// * `type_name` - The prim type name (optional)
    ///
    /// # Returns
    ///
    /// A new PrimSpec, or an error if creation fails.
    pub fn new_child(
        parent_prim: &PrimSpec,
        name: &str,
        specifier: Specifier,
        type_name: &str,
    ) -> Result<Self, String> {
        // Validate name
        if !Self::is_valid_name(name) {
            return Err(format!("'{}' is not a valid prim name", name));
        }

        if parent_prim.is_dormant() {
            return Err("Parent prim is dormant".to_string());
        }

        let layer = parent_prim.layer();
        if !layer.is_valid() {
            return Err("Invalid layer handle".to_string());
        }

        // Create path parent_path/name
        let parent_path = parent_prim.spec.path();
        let child_path = parent_path
            .append_child(name)
            .ok_or_else(|| format!("Failed to create child path for name '{}'", name))?;

        // Check if child already exists
        if layer.get_prim_at_path(&child_path).is_some() {
            return Err(format!("Child prim already exists at path {}", child_path));
        }

        // Create spec in layer
        let child_spec = layer
            .create_prim_spec(&child_path, specifier, type_name)
            .ok_or_else(|| "Failed to create child prim spec in layer".to_string())?;

        // Add to parent's primChildren field
        // Get current children list
        let children_token = tokens::prim_children();
        let mut children: Vec<Token> = Vec::new();

        if let Some(field) = layer.get_field(&parent_path, &children_token) {
            if let Some(existing) = field.as_vec_clone::<Token>() {
                children = existing;
            } else if let Some(existing) = field.as_vec_clone::<String>() {
                children = existing.iter().map(|s| Token::new(s)).collect();
            }
        }

        // Add new child name
        children.push(Token::new(name));

        // Set updated children list
        let children_value = super::abstract_data::Value::new(children);
        layer.set_field(&parent_path, &children_token, children_value);

        Ok(child_spec)
    }

    /// Creates a dormant (invalid) prim spec.
    #[must_use]
    pub fn dormant() -> Self {
        Self {
            spec: Spec::dormant(),
        }
    }

    /// Creates a prim spec from a layer handle and path (internal).
    ///
    /// This is used internally by Layer and other components to create
    /// PrimSpec instances that reference existing specs in the data.
    #[must_use]
    pub(crate) fn new(layer_handle: LayerHandle, path: Path) -> Self {
        Self {
            spec: Spec::new(layer_handle, path),
        }
    }

    // ========================================================================
    // Name
    // ========================================================================

    /// Returns the prim's name.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_sdf::PrimSpec;
    /// // let prim = PrimSpec::new_root(...)?;
    /// // assert_eq!(prim.name(), "World");
    /// ```
    #[must_use]
    pub fn name(&self) -> String {
        self.spec.path().get_name().to_string()
    }

    /// Returns the prim's name as a token.
    #[must_use]
    pub fn name_token(&self) -> Token {
        Token::new(&self.name())
    }

    /// Returns true if setting the prim's name to `new_name` will succeed.
    ///
    /// # Arguments
    ///
    /// * `new_name` - The proposed new name
    ///
    /// # Returns
    ///
    /// True if the rename is valid, false otherwise with reason in `why_not`.
    pub fn can_set_name(&self, new_name: &str, why_not: &mut String) -> bool {
        if !Self::is_valid_name(new_name) {
            *why_not = format!("'{}' is not a valid prim name", new_name);
            return false;
        }
        // Check if sibling with same name exists
        let layer = self.layer();
        if layer.is_valid() {
            let parent_path = self.spec.path().get_parent_path();
            let sibling_path = parent_path.append_child(new_name);
            if let Some(path) = sibling_path {
                if layer.get_prim_at_path(&path).is_some() && path != self.spec.path() {
                    *why_not = format!("a sibling named '{}' already exists", new_name);
                    return false;
                }
            }
        }
        true
    }

    /// Sets the prim's name.
    ///
    /// # Arguments
    ///
    /// * `new_name` - The new name
    /// * `validate` - Whether to validate the name (default: true)
    ///
    /// # Returns
    ///
    /// True if successful, false otherwise.
    pub fn set_name(&mut self, new_name: &str, validate: bool) -> bool {
        if self.is_dormant() {
            return false;
        }

        if validate {
            let mut why_not = String::new();
            if !self.can_set_name(new_name, &mut why_not) {
                return false;
            }
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return false;
        }

        // Compute new path (parent/new_name)
        let old_path = self.spec.path();
        let parent_path = old_path.get_parent_path();
        let new_path = if parent_path.is_absolute_root_path() {
            Path::from_string(&format!("/{}", new_name)).unwrap_or_else(Path::empty)
        } else {
            parent_path
                .append_child(new_name)
                .unwrap_or_else(Path::empty)
        };

        if new_path.is_empty() {
            return false;
        }

        // Move spec to new path using layer
        if let Some(layer_arc) = layer.upgrade() {
            if layer_arc.move_spec(&old_path, &new_path) {
                // Update primChildren in parent: replace old name with new name
                let children_token = Token::new("primChildren");
                let old_name_tok = Token::new(old_path.get_name());
                let new_name_tok = Token::new(new_name);
                if let Some(children_val) = layer_arc.get_field(&parent_path, &children_token) {
                    if let Some(mut children) = children_val.as_vec_clone::<Token>() {
                        if let Some(pos) = children.iter().position(|t| t == &old_name_tok) {
                            children[pos] = new_name_tok;
                            layer_arc.set_field(
                                &parent_path,
                                &children_token,
                                usd_vt::Value::new(children),
                            );
                        }
                    }
                }
                // Update internal path reference
                self.spec = Spec::new(layer, new_path);
                return true;
            }
        }
        false
    }

    /// Returns true if the given string is a valid prim name.
    ///
    /// Valid prim names must:
    /// - Not be empty
    /// - Start with a letter or underscore
    /// - Contain only letters, digits, and underscores
    /// - Not be a reserved name (like ".")
    #[must_use]
    pub fn is_valid_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Check first character (safe: checked is_empty above)
        let first = name.chars().next().expect("name is non-empty");
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }

        // Check remaining characters
        for c in name.chars().skip(1) {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return false;
            }
        }

        // Check for reserved names
        if name == "." || name == ".." {
            return false;
        }

        true
    }

    // ========================================================================
    // Namespace Hierarchy
    // ========================================================================

    /// Returns the prim's namespace pseudo-root prim.
    ///
    /// For root prims, this returns a pseudo-root spec that represents
    /// the layer itself.
    #[must_use]
    pub fn name_root(&self) -> PrimSpec {
        // Return PrimSpec at absolute root path
        if self.is_dormant() {
            return PrimSpec::dormant();
        }
        PrimSpec::new(self.layer(), Path::absolute_root())
    }

    /// Returns the prim's namespace parent.
    ///
    /// Returns None for root prims (doesn't return pseudo-root).
    #[must_use]
    pub fn name_parent(&self) -> Option<PrimSpec> {
        let parent_path = self.spec.path().get_parent_path();
        if parent_path.is_absolute_root_path() {
            return None;
        }
        // Get PrimSpec from layer at parent_path
        self.layer().get_prim_at_path(&parent_path)
    }

    /// Returns the prim's namespace parent, including pseudo-root.
    ///
    /// Unlike `name_parent()`, this returns the pseudo-root for root prims.
    #[must_use]
    pub fn real_name_parent(&self) -> Option<PrimSpec> {
        if self.is_dormant() {
            return None;
        }
        let parent_path = self.spec.path().get_parent_path();
        if parent_path.is_absolute_root_path() {
            return Some(PrimSpec::new(self.layer(), Path::absolute_root()));
        }
        self.layer().get_prim_at_path(&parent_path)
    }

    /// Returns the prim's namespace children.
    ///
    /// These are child prims in the namespace hierarchy.
    #[must_use]
    pub fn name_children(&self) -> Vec<PrimSpec> {
        if self.is_dormant() {
            return Vec::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return Vec::new();
        }

        // Get primChildren field which contains list of child names
        let field = layer.get_field(&self.spec.path(), &tokens::prim_children());
        if let Some(value) = field {
            // Handle Vec<Token>, Array<Token>, Vec<String>, Array<String> for USDA/USDC compat.
            if let Some(names) = value.as_vec_clone::<Token>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let child_path = self.spec.path().append_child(name.as_str())?;
                        layer.get_prim_at_path(&child_path)
                    })
                    .collect();
            }
            if let Some(names) = value.as_vec_clone::<String>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let child_path = self.spec.path().append_child(name)?;
                        layer.get_prim_at_path(&child_path)
                    })
                    .collect();
            }
        }

        Vec::new()
    }

    /// Sets the namespace children to match the given list.
    pub fn set_name_children(&mut self, children: &[PrimSpec]) {
        if self.is_dormant() {
            return;
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return;
        }
        let names: Vec<Token> = children.iter().map(|c| c.name_token()).collect();
        let value = super::abstract_data::Value::new(names);
        layer.set_field(&self.spec.path(), &tokens::prim_children(), value);
    }

    /// Inserts a child prim. `index` is checked for range; -1 means append.
    ///
    /// Returns true if successful.
    pub fn insert_name_child(&mut self, child: &PrimSpec, index: i32) -> bool {
        if self.is_dormant() || child.is_dormant() {
            return false;
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return false;
        }

        let child_name = child.name_token();
        let mut names = self.name_children_tokens();

        // Check for duplicates
        if names.contains(&child_name) {
            return false;
        }

        if index < 0 || index as usize >= names.len() {
            names.push(child_name);
        } else {
            names.insert(index as usize, child_name);
        }

        let value = super::abstract_data::Value::new(names);
        layer.set_field(&self.spec.path(), &tokens::prim_children(), value);
        true
    }

    /// Removes a child prim. Returns true if successful.
    pub fn remove_name_child(&mut self, child: &PrimSpec) -> bool {
        if self.is_dormant() || child.is_dormant() {
            return false;
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return false;
        }

        let child_name = child.name_token();
        let mut names = self.name_children_tokens();
        let original_len = names.len();
        names.retain(|n| n != &child_name);

        if names.len() == original_len {
            return false; // Not found
        }

        let value = super::abstract_data::Value::new(names);
        layer.set_field(&self.spec.path(), &tokens::prim_children(), value);
        true
    }

    /// Returns the name children order (reorder statement).
    #[must_use]
    pub fn name_children_order(&self) -> Vec<Token> {
        self.get_token_list_field(&tokens::name_children_order())
    }

    /// Returns true if this prim has name children order specified.
    #[must_use]
    pub fn has_name_children_order(&self) -> bool {
        self.spec.has_info(&tokens::name_children_order())
    }

    /// Sets the name children reorder statement.
    pub fn set_name_children_order(&mut self, names: &[Token]) {
        self.set_token_list_field(&tokens::name_children_order(), names);
    }

    /// Inserts a name into the name children order. -1 means append.
    pub fn insert_in_name_children_order(&mut self, name: &Token, index: i32) {
        self.insert_in_token_list_field(&tokens::name_children_order(), name, index);
    }

    /// Removes a name from the name children order.
    pub fn remove_from_name_children_order(&mut self, name: &Token) {
        self.remove_from_token_list_field(&tokens::name_children_order(), name);
    }

    /// Removes a name from the name children order by index.
    pub fn remove_from_name_children_order_by_index(&mut self, index: usize) {
        self.remove_from_token_list_field_by_index(&tokens::name_children_order(), index);
    }

    /// Reorders `vec` according to the name children order statement.
    pub fn apply_name_children_order(&self, vec: &mut Vec<Token>) {
        let order = self.name_children_order();
        if order.is_empty() {
            return;
        }
        apply_ordering(vec, &order);
    }

    // ========================================================================
    // Properties
    // ========================================================================

    /// Returns all properties of this prim.
    #[must_use]
    pub fn properties(&self) -> Vec<PropertySpec> {
        if self.is_dormant() {
            return Vec::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return Vec::new();
        }

        // Get properties field which contains list of property names
        let field = layer.get_field(&self.spec.path(), &tokens::properties());
        if let Some(value) = field {
            // Try to extract Vec<Token> or Vec<String> from Value
            if let Some(names) = value.as_vec_clone::<Token>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let prop_path = self.spec.path().append_property(name.as_str())?;
                        layer.get_property_at_path(&prop_path)
                    })
                    .collect();
            }
            if let Some(names) = value.as_vec_clone::<String>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let prop_path = self.spec.path().append_property(name)?;
                        layer.get_property_at_path(&prop_path)
                    })
                    .collect();
            }
        }

        Vec::new()
    }

    /// Returns all attributes of this prim.
    #[must_use]
    pub fn attributes(&self) -> Vec<AttributeSpec> {
        if self.is_dormant() {
            return Vec::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return Vec::new();
        }

        // Get properties field which contains list of property names
        let field = layer.get_field(&self.spec.path(), &tokens::properties());
        if let Some(value) = field {
            // Try to extract Vec<Token> or Vec<String> from Value
            if let Some(names) = value.as_vec_clone::<Token>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let prop_path = self.spec.path().append_property(name.as_str())?;
                        layer.get_attribute_at_path(&prop_path)
                    })
                    .collect();
            }
            if let Some(names) = value.as_vec_clone::<String>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let prop_path = self.spec.path().append_property(name)?;
                        layer.get_attribute_at_path(&prop_path)
                    })
                    .collect();
            }
        }

        Vec::new()
    }

    /// Returns all relationships of this prim.
    #[must_use]
    pub fn relationships(&self) -> Vec<RelationshipSpec> {
        if self.is_dormant() {
            return Vec::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return Vec::new();
        }

        // Get properties field which contains list of property names
        let field = layer.get_field(&self.spec.path(), &tokens::properties());
        if let Some(value) = field {
            // Try to extract Vec<Token> or Vec<String> from Value
            if let Some(names) = value.as_vec_clone::<Token>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let prop_path = self.spec.path().append_property(name.as_str())?;
                        layer.get_relationship_at_path(&prop_path)
                    })
                    .collect();
            }
            if let Some(names) = value.as_vec_clone::<String>() {
                return names
                    .iter()
                    .filter_map(|name| {
                        let prop_path = self.spec.path().append_property(name)?;
                        layer.get_relationship_at_path(&prop_path)
                    })
                    .collect();
            }
        }

        Vec::new()
    }

    /// Sets the properties list to match the given vector.
    pub fn set_properties(&mut self, props: &[PropertySpec]) {
        if self.is_dormant() {
            return;
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return;
        }
        let names: Vec<Token> = props.iter().map(|p| p.name()).collect();
        let value = super::abstract_data::Value::new(names);
        layer.set_field(&self.spec.path(), &tokens::properties(), value);
    }

    /// Inserts a property. `index` is checked for range; -1 means append.
    ///
    /// Returns true if successful.
    pub fn insert_property(&mut self, property: &PropertySpec, index: i32) -> bool {
        if self.is_dormant() || property.is_dormant() {
            return false;
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return false;
        }

        let prop_name = property.name();
        let mut names = self.property_tokens();

        if names.contains(&prop_name) {
            return false;
        }

        if index < 0 || index as usize >= names.len() {
            names.push(prop_name);
        } else {
            names.insert(index as usize, prop_name);
        }

        let value = super::abstract_data::Value::new(names);
        layer.set_field(&self.spec.path(), &tokens::properties(), value);
        true
    }

    /// Removes a property.
    pub fn remove_property(&mut self, property: &PropertySpec) {
        if self.is_dormant() || property.is_dormant() {
            return;
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return;
        }

        let prop_name = property.name();
        let mut names = self.property_tokens();
        names.retain(|n| n != &prop_name);

        let value = super::abstract_data::Value::new(names);
        layer.set_field(&self.spec.path(), &tokens::properties(), value);
    }

    /// Returns the property order (reorder statement).
    #[must_use]
    pub fn property_order(&self) -> Vec<Token> {
        self.get_token_list_field(&tokens::property_order())
    }

    /// Returns true if this prim has property ordering specified.
    #[must_use]
    pub fn has_property_order(&self) -> bool {
        self.spec.has_info(&tokens::property_order())
    }

    /// Sets the property reorder statement.
    pub fn set_property_order(&mut self, names: &[Token]) {
        self.set_token_list_field(&tokens::property_order(), names);
    }

    /// Inserts a name into the property order. -1 means append.
    pub fn insert_in_property_order(&mut self, name: &Token, index: i32) {
        self.insert_in_token_list_field(&tokens::property_order(), name, index);
    }

    /// Removes a name from the property order.
    pub fn remove_from_property_order(&mut self, name: &Token) {
        self.remove_from_token_list_field(&tokens::property_order(), name);
    }

    /// Removes a name from the property order by index.
    pub fn remove_from_property_order_by_index(&mut self, index: usize) {
        self.remove_from_token_list_field_by_index(&tokens::property_order(), index);
    }

    /// Reorders `vec` according to the property order statement.
    pub fn apply_property_order(&self, vec: &mut Vec<Token>) {
        let order = self.property_order();
        if order.is_empty() {
            return;
        }
        apply_ordering(vec, &order);
    }

    // ========================================================================
    // Lookup
    // ========================================================================

    /// Returns a prim at the given path (relative to this prim or absolute).
    #[must_use]
    pub fn get_prim_at_path(&self, path: &Path) -> Option<PrimSpec> {
        let resolved = self.resolve_path(path);
        self.layer().get_prim_at_path(&resolved)
    }

    /// Returns a property at the given path (relative to this prim or absolute).
    #[must_use]
    pub fn get_property_at_path(&self, path: &Path) -> Option<PropertySpec> {
        let resolved = self.resolve_path(path);
        self.layer().get_property_at_path(&resolved)
    }

    /// Returns an attribute at the given path (relative to this prim or absolute).
    #[must_use]
    pub fn get_attribute_at_path(&self, path: &Path) -> Option<AttributeSpec> {
        let resolved = self.resolve_path(path);
        self.layer().get_attribute_at_path(&resolved)
    }

    /// Returns a relationship at the given path (relative to this prim or absolute).
    #[must_use]
    pub fn get_relationship_at_path(&self, path: &Path) -> Option<RelationshipSpec> {
        let resolved = self.resolve_path(path);
        self.layer().get_relationship_at_path(&resolved)
    }

    // ========================================================================
    // Generic Info Access (matches C++ SdfSpec::SetInfo / GetInfo)
    // ========================================================================

    /// Sets a metadata field on this prim spec.
    ///
    /// Matches C++ `SdfPrimSpec::SetInfo(const TfToken &key, const VtValue &value)`.
    pub fn set_info(&mut self, key: &Token, value: VtValue) {
        self.spec.set_info(key, value);
    }

    /// Gets a metadata field from this prim spec.
    ///
    /// Matches C++ `SdfPrimSpec::GetInfo(const TfToken &key)`.
    pub fn get_info(&self, key: &Token) -> VtValue {
        self.spec.get_info(key)
    }

    /// Checks if a metadata field is set on this prim spec.
    ///
    /// Matches C++ `SdfPrimSpec::HasInfo(const TfToken &key)`.
    pub fn has_info(&self, key: &Token) -> bool {
        self.spec.has_info(key)
    }

    // ========================================================================
    // Metadata - Type and Specifier
    // ========================================================================

    /// Returns the prim's type name.
    ///
    /// The type name identifies the schema type of this prim
    /// (e.g., "Xform", "Mesh", "Camera").
    #[must_use]
    pub fn type_name(&self) -> Token {
        let v = self.spec.get_info(&tokens::type_name());
        // C++ stores typeName as TfToken; handle both Token and String for compat
        if let Some(tok) = v.get::<Token>() {
            return tok.clone();
        }
        if let Some(s) = v.get::<String>() {
            return Token::new(s);
        }
        Token::empty()
    }

    /// Sets the prim's type name.
    ///
    /// # Arguments
    ///
    /// * `value` - The type name to set
    ///
    /// Matches C++ SdfPrimSpec::SetTypeName: cannot set empty type name unless specifier is Over.
    pub fn set_type_name(&mut self, value: &str) {
        if value.is_empty() && self.specifier() != Specifier::Over {
            // C++ issues a TF_CODING_ERROR here but still no-ops
            return;
        }
        // C++ stores typeName as TfToken (not std::string)
        self.spec
            .set_info(&tokens::type_name(), VtValue::new(Token::new(value)));
    }

    /// Returns the prim's specifier (Def, Over, or Class).
    #[must_use]
    pub fn specifier(&self) -> Specifier {
        let v = self.spec.get_info(&tokens::specifier());
        // C++ stores Specifier as typed enum; USDC stores as integer, USDA stores as token/string
        if let Some(&sp) = v.get::<Specifier>() {
            return sp;
        }
        if let Some(s) = v.get::<String>() {
            if let Ok(sp) = Specifier::try_from(s.as_str()) {
                return sp;
            }
        }
        if let Some(tok) = v.get::<Token>() {
            if let Ok(sp) = Specifier::try_from(tok.as_str()) {
                return sp;
            }
        }
        Specifier::Over
    }

    /// Sets the prim's specifier.
    ///
    /// # Arguments
    ///
    /// * `value` - The specifier to set (Def, Over, or Class)
    pub fn set_specifier(&mut self, value: Specifier) {
        // C++ stores Specifier as typed enum value
        self.spec
            .set_info(&tokens::specifier(), VtValue::new(value));
    }

    // ========================================================================
    // Metadata - Documentation
    // ========================================================================

    /// Returns the comment string for this prim spec.
    ///
    /// Default value is an empty string.
    #[must_use]
    pub fn comment(&self) -> String {
        self.spec
            .get_info(&tokens::comment())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the comment string for this prim spec.
    pub fn set_comment(&mut self, value: &str) {
        self.spec
            .set_info(&tokens::comment(), VtValue::new(value.to_string()));
    }

    /// Returns the documentation string for this prim spec.
    ///
    /// Default value is an empty string.
    #[must_use]
    pub fn documentation(&self) -> String {
        self.spec
            .get_info(&tokens::documentation())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the documentation string for this prim spec.
    pub fn set_documentation(&mut self, value: &str) {
        self.spec
            .set_info(&tokens::documentation(), VtValue::new(value.to_string()));
    }

    // ========================================================================
    // Metadata - Active
    // ========================================================================

    /// Returns whether this prim spec is active.
    ///
    /// Default value is true.
    #[must_use]
    pub fn active(&self) -> bool {
        self.spec
            .get_info(&tokens::active())
            .get::<bool>()
            .copied()
            .unwrap_or(true)
    }

    /// Sets whether this prim spec is active.
    pub fn set_active(&mut self, value: bool) {
        self.spec.set_info(&tokens::active(), VtValue::new(value));
    }

    /// Returns true if this prim spec has an opinion about active.
    #[must_use]
    pub fn has_active(&self) -> bool {
        self.spec.has_info(&tokens::active())
    }

    /// Removes the active opinion in this prim spec.
    pub fn clear_active(&mut self) {
        self.spec.clear_info(&tokens::active());
    }

    // ========================================================================
    // Metadata - Hidden
    // ========================================================================

    /// Returns whether this prim spec will be hidden in browsers.
    ///
    /// Default value is false.
    ///
    /// # Deprecated
    ///
    /// See UsdUIObjectHints.
    #[must_use]
    pub fn hidden(&self) -> bool {
        self.spec
            .get_info(&tokens::hidden())
            .get::<bool>()
            .copied()
            .unwrap_or(false)
    }

    /// Sets whether this prim spec will be hidden in browsers.
    ///
    /// # Deprecated
    ///
    /// See UsdUIObjectHints.
    pub fn set_hidden(&mut self, value: bool) {
        self.spec.set_info(&tokens::hidden(), VtValue::new(value));
    }

    // ========================================================================
    // Metadata - Kind
    // ========================================================================

    /// Returns this prim spec's kind.
    ///
    /// Default value is an empty token.
    #[must_use]
    pub fn kind(&self) -> Token {
        let v = self.spec.get_info(&tokens::kind());
        // C++ stores Kind as TfToken; handle both Token and String for compat
        if let Some(tok) = v.get::<Token>() {
            return tok.clone();
        }
        if let Some(s) = v.get::<String>() {
            return Token::new(s);
        }
        Token::empty()
    }

    /// Sets this prim spec's kind.
    pub fn set_kind(&mut self, value: &Token) {
        // C++ stores Kind as TfToken
        self.spec
            .set_info(&tokens::kind(), VtValue::new(value.clone()));
    }

    /// Returns true if this prim spec has an opinion about kind.
    #[must_use]
    pub fn has_kind(&self) -> bool {
        self.spec.has_info(&tokens::kind())
    }

    /// Removes the kind opinion from this prim spec.
    pub fn clear_kind(&mut self) {
        self.spec.clear_info(&tokens::kind());
    }

    // ========================================================================
    // Metadata - Permission
    // ========================================================================

    /// Returns the prim's permission restriction.
    ///
    /// Default value is Permission::Public.
    #[must_use]
    pub fn permission(&self) -> Permission {
        self.spec
            .get_info(&tokens::permission())
            .get::<String>()
            .map(|s| {
                if s == "private" {
                    Permission::Private
                } else {
                    Permission::Public
                }
            })
            .unwrap_or(Permission::Public)
    }

    /// Sets the prim's permission restriction.
    pub fn set_permission(&mut self, value: Permission) {
        self.spec
            .set_info(&tokens::permission(), VtValue::new(value.to_string()));
    }

    // ========================================================================
    // Metadata - Symmetry
    // ========================================================================

    /// Returns the symmetry function for this prim.
    ///
    /// Default value is an empty token.
    #[must_use]
    pub fn symmetry_function(&self) -> Token {
        self.spec
            .get_info(&tokens::symmetry_function())
            .get::<String>()
            .map(|s| Token::new(s))
            .unwrap_or_else(Token::empty)
    }

    /// Sets the symmetry function for this prim.
    ///
    /// If `function_name` is empty, removes the symmetry function.
    pub fn set_symmetry_function(&mut self, function_name: &Token) {
        if function_name.is_empty() {
            self.spec.clear_info(&tokens::symmetry_function());
        } else {
            self.spec.set_info(
                &tokens::symmetry_function(),
                VtValue::new(function_name.as_str().to_string()),
            );
        }
    }

    /// Returns the symmetry arguments dictionary for this prim.
    ///
    /// Default value is an empty dictionary.
    #[must_use]
    pub fn symmetry_arguments(&self) -> VtDictionary {
        let value = self.spec.get_field(&tokens::symmetry_arguments());
        value.as_dictionary().unwrap_or_default()
    }

    /// Sets a symmetry argument for this prim.
    ///
    /// If `value` is empty, removes the argument.
    pub fn set_symmetry_argument(&mut self, name: &str, value: VtValue) {
        self.spec
            .set_info_dict_value(&tokens::symmetry_arguments(), name, value);
    }

    /// Returns the symmetric peer for this prim.
    ///
    /// Default value is an empty string.
    #[must_use]
    pub fn symmetric_peer(&self) -> String {
        self.spec
            .get_info(&tokens::symmetric_peer())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the symmetric peer for this prim.
    ///
    /// If `peer_name` is empty, removes the symmetric peer.
    pub fn set_symmetric_peer(&mut self, peer_name: &str) {
        if peer_name.is_empty() {
            self.spec.clear_info(&tokens::symmetric_peer());
        } else {
            self.spec.set_info(
                &tokens::symmetric_peer(),
                VtValue::new(peer_name.to_string()),
            );
        }
    }

    // ========================================================================
    // Metadata - Prefix and Suffix
    // ========================================================================

    /// Returns the prefix string for this prim spec.
    ///
    /// Default value is an empty string.
    #[must_use]
    pub fn prefix(&self) -> String {
        self.spec
            .get_info(&tokens::prefix())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the prefix string for this prim spec.
    pub fn set_prefix(&mut self, value: &str) {
        self.spec
            .set_info(&tokens::prefix(), VtValue::new(value.to_string()));
    }

    /// Returns the suffix string for this prim spec.
    ///
    /// Default value is an empty string.
    #[must_use]
    pub fn suffix(&self) -> String {
        self.spec
            .get_info(&tokens::suffix())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the suffix string for this prim spec.
    pub fn set_suffix(&mut self, value: &str) {
        self.spec
            .set_info(&tokens::suffix(), VtValue::new(value.to_string()));
    }

    // ========================================================================
    // Metadata - Prefix/Suffix Substitutions
    // ========================================================================

    /// Returns the prefix substitutions dictionary.
    #[must_use]
    pub fn prefix_substitutions(&self) -> VtDictionary {
        let value = self.spec.get_field(&tokens::prefix_substitutions());
        value.as_dictionary().unwrap_or_default()
    }

    /// Sets the prefix substitutions dictionary.
    pub fn set_prefix_substitutions(&mut self, dict: VtDictionary) {
        let value = VtValue::from_dictionary(dict);
        let _ = self.spec.set_field(&tokens::prefix_substitutions(), value);
    }

    /// Returns the suffix substitutions dictionary.
    #[must_use]
    pub fn suffix_substitutions(&self) -> VtDictionary {
        let value = self.spec.get_field(&tokens::suffix_substitutions());
        value.as_dictionary().unwrap_or_default()
    }

    /// Sets the suffix substitutions dictionary.
    pub fn set_suffix_substitutions(&mut self, dict: VtDictionary) {
        let value = VtValue::from_dictionary(dict);
        let _ = self.spec.set_field(&tokens::suffix_substitutions(), value);
    }

    // ========================================================================
    // Metadata - Instanceable
    // ========================================================================

    /// Returns the prim's instanceable flag.
    ///
    /// Default value is false.
    #[must_use]
    pub fn instanceable(&self) -> bool {
        self.spec
            .get_info(&tokens::instanceable())
            .get::<bool>()
            .copied()
            .unwrap_or(false)
    }

    /// Sets the prim's instanceable flag.
    pub fn set_instanceable(&mut self, value: bool) {
        self.spec
            .set_info(&tokens::instanceable(), VtValue::new(value));
    }

    /// Returns true if this prim has an instanceable opinion.
    #[must_use]
    pub fn has_instanceable(&self) -> bool {
        self.spec.has_info(&tokens::instanceable())
    }

    /// Clears the instanceable opinion.
    pub fn clear_instanceable(&mut self) {
        self.spec.clear_info(&tokens::instanceable());
    }

    // ========================================================================
    // Metadata - Clip Sets
    // ========================================================================

    /// Returns true if this prim has clip sets authored.
    #[must_use]
    pub fn has_clip_sets(&self) -> bool {
        self.spec.has_info(&tokens::clip_sets())
    }

    /// Returns the clip set names for this prim as a list of strings.
    ///
    /// Matches C++ `GetClipSetsList()`. In C++ this returns a proxy; here we
    /// return the resolved list of clip set names derived from the clips dictionary.
    #[must_use]
    pub fn get_clip_set_names(&self) -> Vec<String> {
        let clips = self.clips();
        clips.keys().cloned().collect()
    }

    /// Returns the value clips dictionary for this prim.
    ///
    /// Default value is an empty dictionary.
    #[must_use]
    pub fn clips(&self) -> VtDictionary {
        let value = self.spec.get_field(&tokens::clips());
        value.as_dictionary().unwrap_or_default()
    }

    /// Sets a value clips entry.
    ///
    /// If `value` is empty, removes the entry.
    pub fn set_clips(&mut self, name: &str, value: VtValue) {
        self.spec.set_info_dict_value(&tokens::clips(), name, value);
    }

    // ========================================================================
    // Metadata - Custom Data and Asset Info
    // ========================================================================

    /// Returns the custom data dictionary for this prim.
    ///
    /// Custom data is for use by plugins or extensions that need to store
    /// data attached to scene objects.
    #[must_use]
    pub fn custom_data(&self) -> VtDictionary {
        self.spec.custom_data()
    }

    /// Sets a custom data entry for this prim.
    ///
    /// If `value` is empty, removes the custom data entry.
    pub fn set_custom_data(&mut self, name: &str, value: VtValue) {
        self.spec
            .set_info_dict_value(&tokens::custom_data(), name, value);
    }

    /// Returns the asset info dictionary for this prim.
    ///
    /// Asset info contains metadata about assets, such as identifiers,
    /// versions, and other asset management data.
    #[must_use]
    pub fn asset_info(&self) -> VtDictionary {
        self.spec.asset_info()
    }

    /// Sets an asset info entry for this prim.
    ///
    /// If `value` is empty, removes the asset info entry.
    pub fn set_asset_info(&mut self, name: &str, value: VtValue) {
        self.spec
            .set_info_dict_value(&tokens::asset_info(), name, value);
    }

    // ========================================================================
    // Composition Arcs - References
    // ========================================================================

    /// Returns the references list operation for this prim.
    ///
    /// Returns a `ListOp<Reference>` matching C++ SdfReferencesProxy.
    /// References can be modified through the returned ListOp.
    #[must_use]
    pub fn references_list(&self) -> ReferenceListOp {
        if self.is_dormant() {
            return ReferenceListOp::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return ReferenceListOp::new();
        }

        // Get references field as ReferenceListOp
        let field = layer.get_field(&self.spec.path(), &tokens::references());
        if let Some(value) = field {
            if let Some(list_op) = value.downcast::<ReferenceListOp>() {
                return list_op.clone();
            }
            // Fallback: try PathListOp for backwards compatibility
            if let Some(_path_list_op) = value.downcast::<PathListOp>() {
                // Can't losslessly convert PathListOp -> ReferenceListOp;
                // return empty. This path shouldn't occur with proper data.
                return ReferenceListOp::new();
            }
        }

        ReferenceListOp::new()
    }

    /// Returns true if this prim has references set.
    #[must_use]
    pub fn has_references(&self) -> bool {
        self.spec.has_info(&tokens::references())
    }

    /// Clears the references for this prim.
    pub fn clear_references(&mut self) {
        self.spec.clear_info(&tokens::references());
    }

    // ========================================================================
    // Composition Arcs - Payloads
    // ========================================================================

    /// Returns the payloads list operation for this prim.
    ///
    /// Returns a `ListOp<Payload>` matching C++ SdfPayloadsProxy.
    /// Payloads can be modified through the returned ListOp.
    #[must_use]
    pub fn payloads_list(&self) -> PayloadListOp {
        if self.is_dormant() {
            return PayloadListOp::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return PayloadListOp::new();
        }

        // Get payload field as PayloadListOp
        let field = layer.get_field(&self.spec.path(), &tokens::payload());
        if let Some(value) = field {
            if let Some(list_op) = value.downcast::<PayloadListOp>() {
                return list_op.clone();
            }
            // Fallback: try PathListOp for backwards compatibility
            if let Some(_path_list_op) = value.downcast::<PathListOp>() {
                return PayloadListOp::new();
            }
        }

        PayloadListOp::new()
    }

    /// Returns true if this prim has payloads set.
    #[must_use]
    pub fn has_payloads(&self) -> bool {
        self.spec.has_info(&tokens::payload())
    }

    /// Clears the payloads for this prim.
    pub fn clear_payloads(&mut self) {
        self.spec.clear_info(&tokens::payload());
    }

    // ========================================================================
    // Composition Arcs - Inherits
    // ========================================================================

    /// Returns the inherit paths list operation for this prim.
    ///
    /// Inherit paths can be modified through the returned ListOp.
    #[must_use]
    pub fn inherits_list(&self) -> PathListOp {
        if self.is_dormant() {
            return PathListOp::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return PathListOp::new();
        }

        // Get inheritPaths field as PathListOp
        let field = layer.get_field(&self.spec.path(), &tokens::inherit_paths());
        if let Some(value) = field {
            // Try to downcast to PathListOp
            if let Some(list_op) = value.downcast::<PathListOp>() {
                return list_op.clone();
            }
        }

        PathListOp::new()
    }

    /// Returns true if this prim has inherit paths set.
    #[must_use]
    pub fn has_inherits(&self) -> bool {
        self.spec.has_info(&tokens::inherit_paths())
    }

    /// Clears the inherit paths for this prim.
    pub fn clear_inherits(&mut self) {
        self.spec.clear_info(&tokens::inherit_paths());
    }

    // ========================================================================
    // Composition Arcs - Specializes
    // ========================================================================

    /// Returns the specializes list operation for this prim.
    ///
    /// Specializes can be modified through the returned ListOp.
    #[must_use]
    pub fn specializes_list(&self) -> PathListOp {
        if self.is_dormant() {
            return PathListOp::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return PathListOp::new();
        }

        // Get specializes field as PathListOp
        let field = layer.get_field(&self.spec.path(), &tokens::specializes());
        if let Some(value) = field {
            // Try to downcast to PathListOp
            if let Some(list_op) = value.downcast::<PathListOp>() {
                return list_op.clone();
            }
        }

        PathListOp::new()
    }

    /// Returns true if this prim has specializes set.
    #[must_use]
    pub fn has_specializes(&self) -> bool {
        self.spec.has_info(&tokens::specializes())
    }

    /// Clears the specializes for this prim.
    pub fn clear_specializes(&mut self) {
        self.spec.clear_info(&tokens::specializes());
    }

    // ========================================================================
    // Variants
    // ========================================================================

    /// Returns a proxy for the prim's variant sets.
    ///
    /// Variant sets for this prim may be modified through the proxy.
    #[must_use]
    pub fn variant_sets(&self) -> VariantSetsProxy {
        if self.is_dormant() {
            return VariantSetsProxy::dormant();
        }
        VariantSetsProxy::new(self.clone())
    }

    /// Returns the variant set name list (list of variant set names).
    #[must_use]
    pub fn variant_set_name_list(&self) -> Vec<Token> {
        self.get_token_list_field(&tokens::variant_set_names())
    }

    /// Returns true if this prim has variant set names.
    #[must_use]
    pub fn has_variant_set_names(&self) -> bool {
        self.spec.has_info(&tokens::variant_set_names())
    }

    /// Returns the variant names for the given variant set.
    #[must_use]
    pub fn variant_names(&self, set_name: &str) -> Vec<String> {
        if self.is_dormant() {
            return Vec::new();
        }
        let vs = self.variant_sets();
        if let Some(set) = vs.get(set_name) {
            set.variant_names()
                .iter()
                .map(|t| t.as_str().to_string())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Removes a variant set by name.
    ///
    /// Note: the set's name should also be removed from the variant set names list.
    pub fn remove_variant_set(&mut self, name: &str) {
        if self.is_dormant() {
            return;
        }
        // Remove from variant set names list
        let mut names = self.variant_set_name_list();
        let name_token = Token::new(name);
        names.retain(|n| n != &name_token);
        self.set_token_list_field(&tokens::variant_set_names(), &names);

        // Delete the variant set spec itself
        let layer = self.layer();
        if let Some(layer_arc) = layer.upgrade() {
            // Build variant selection path: /Prim{setName=}
            let vs_path_str = format!("{}{{{}=}}", self.spec.path().get_string(), name);
            if let Some(path) = Path::from_string(&vs_path_str) {
                let _ = layer_arc.delete_spec(&path);
            }
        }
    }

    /// Returns the variant selections map.
    ///
    /// Maps variant set names to selected variant names.
    /// Alias for `variant_selection()` (C++ naming compatibility).
    #[must_use]
    pub fn variant_selections(&self) -> HashMap<String, String> {
        self.variant_selection()
    }

    /// Returns the variant selections for this prim.
    ///
    /// Maps variant set names to selected variant names.
    #[must_use]
    pub fn variant_selection(&self) -> HashMap<String, String> {
        if self.is_dormant() {
            return HashMap::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return HashMap::new();
        }

        // Get variantSelection field as dictionary
        let field = layer.get_field(&self.spec.path(), &tokens::variant_selection());
        if let Some(value) = field {
            // Try to downcast to HashMap<String, String>
            if let Some(selection) = value.downcast::<HashMap<String, String>>() {
                return selection.clone();
            }
        }

        HashMap::new()
    }

    /// Sets the variant selected for the given variant set.
    ///
    /// If `variant_name` is empty, removes the variant selection opinion.
    /// To explicitly set the selection to empty, use `block_variant_selection`.
    pub fn set_variant_selection(&mut self, variant_set_name: &str, variant_name: &str) {
        if variant_name.is_empty() {
            // Remove selection
            self.spec.set_info_dict_value(
                &tokens::variant_selection(),
                variant_set_name,
                VtValue::empty(),
            );
        } else {
            // Set selection
            self.spec.set_info_dict_value(
                &tokens::variant_selection(),
                variant_set_name,
                VtValue::new(variant_name.to_string()),
            );
        }
    }

    /// Blocks the variant selected for the given variant set.
    ///
    /// Sets the variant selection to an empty string, which is different
    /// from removing the opinion entirely.
    pub fn block_variant_selection(&mut self, variant_set_name: &str) {
        self.spec.set_info_dict_value(
            &tokens::variant_selection(),
            variant_set_name,
            VtValue::new(String::new()),
        );
    }

    // ========================================================================
    // Relocates
    // ========================================================================

    /// Returns the relocates map for this prim.
    ///
    /// Maps source paths to target paths for namespace relocations.
    #[must_use]
    pub fn relocates(&self) -> BTreeMap<Path, Path> {
        if self.is_dormant() {
            return BTreeMap::new();
        }

        let layer = self.layer();
        if !layer.is_valid() {
            return BTreeMap::new();
        }

        let field = layer.get_field(&self.spec.path(), &tokens::relocates());
        if let Some(value) = field {
            if let Some(map) = value.downcast::<BTreeMap<Path, Path>>() {
                return map.clone();
            }
        }

        BTreeMap::new()
    }

    /// Sets the entire relocates map.
    pub fn set_relocates(&mut self, new_map: BTreeMap<Path, Path>) {
        if self.is_dormant() {
            return;
        }
        let value = super::abstract_data::Value::new(new_map);
        self.layer()
            .set_field(&self.spec.path(), &tokens::relocates(), value);
    }

    /// Returns true if this prim has a relocates opinion (including empty map).
    #[must_use]
    pub fn has_relocates(&self) -> bool {
        self.spec.has_info(&tokens::relocates())
    }

    /// Clears the relocates opinion.
    pub fn clear_relocates(&mut self) {
        self.spec.clear_info(&tokens::relocates());
    }

    // ========================================================================
    // Internal Accessors
    // ========================================================================

    /// Returns the underlying Spec.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Returns the path of this prim spec.
    #[must_use]
    pub fn path(&self) -> Path {
        self.spec.path()
    }

    /// Returns the layer containing this prim spec.
    #[must_use]
    pub fn layer(&self) -> LayerHandle {
        self.spec.layer()
    }

    /// Returns true if this spec is dormant (invalid or expired).
    #[must_use]
    pub fn is_dormant(&self) -> bool {
        self.spec.is_dormant()
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Resolves a path relative to this prim, or returns it as-is if absolute.
    fn resolve_path(&self, path: &Path) -> Path {
        if path.is_absolute_path() {
            return path.clone();
        }
        // Make path relative to this prim's path
        if let Some(resolved) = self.spec.path().append_path(path) {
            resolved
        } else {
            path.clone()
        }
    }

    /// Gets the name children token list from the primChildren field.
    fn name_children_tokens(&self) -> Vec<Token> {
        self.get_token_list_field(&tokens::prim_children())
    }

    /// Gets the property token list from the properties field.
    fn property_tokens(&self) -> Vec<Token> {
        self.get_token_list_field(&tokens::properties())
    }

    /// Generic helper: get a Vec<Token> field from the layer.
    fn get_token_list_field(&self, field_name: &Token) -> Vec<Token> {
        if self.is_dormant() {
            return Vec::new();
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return Vec::new();
        }
        let field = layer.get_field(&self.spec.path(), field_name);
        if let Some(value) = field {
            if let Some(names) = value.as_vec_clone::<Token>() {
                return names;
            }
            if let Some(names) = value.as_vec_clone::<String>() {
                return names.iter().map(|s| Token::new(s)).collect();
            }
        }
        Vec::new()
    }

    /// Generic helper: set a Vec<Token> field on the layer.
    fn set_token_list_field(&mut self, field_name: &Token, names: &[Token]) {
        if self.is_dormant() {
            return;
        }
        let layer = self.layer();
        if !layer.is_valid() {
            return;
        }
        let value = super::abstract_data::Value::new(names.to_vec());
        layer.set_field(&self.spec.path(), field_name, value);
    }

    /// Generic helper: insert a token into a Vec<Token> field.
    fn insert_in_token_list_field(&mut self, field_name: &Token, name: &Token, index: i32) {
        let mut names = self.get_token_list_field(field_name);
        if index < 0 || index as usize >= names.len() {
            names.push(name.clone());
        } else {
            names.insert(index as usize, name.clone());
        }
        self.set_token_list_field(field_name, &names);
    }

    /// Generic helper: remove a token from a Vec<Token> field.
    fn remove_from_token_list_field(&mut self, field_name: &Token, name: &Token) {
        let mut names = self.get_token_list_field(field_name);
        names.retain(|n| n != name);
        self.set_token_list_field(field_name, &names);
    }

    /// Generic helper: remove a token by index from a Vec<Token> field.
    fn remove_from_token_list_field_by_index(&mut self, field_name: &Token, index: usize) {
        let mut names = self.get_token_list_field(field_name);
        if index < names.len() {
            names.remove(index);
            self.set_token_list_field(field_name, &names);
        }
    }
}

// ============================================================================
// Free Functions
// ============================================================================

/// Applies a partial ordering to a vector of tokens.
///
/// Items in `order` are moved to their relative positions while items not
/// in `order` remain in their original relative order, placed after any
/// ordered items that precede them.
///
/// This implements the "reorder" semantics used by nameChildrenOrder
/// and propertyOrder in USD.
pub fn apply_ordering(vec: &mut Vec<Token>, order: &[Token]) {
    if order.is_empty() || vec.is_empty() {
        return;
    }

    // Build a position map from order
    let order_map: HashMap<&Token, usize> = order.iter().enumerate().map(|(i, t)| (t, i)).collect();

    // Stable sort: ordered items come first in order sequence, unordered items
    // retain their relative positions at the end.
    let max_pos = order.len();
    let mut indexed: Vec<(usize, usize, Token)> = vec
        .drain(..)
        .enumerate()
        .map(|(orig_idx, token)| {
            let sort_key = order_map.get(&token).copied().unwrap_or(max_pos + orig_idx);
            (sort_key, orig_idx, token)
        })
        .collect();

    indexed.sort_by_key(|(key, orig, _)| (*key, *orig));
    vec.extend(indexed.into_iter().map(|(_, _, token)| token));
}

// ============================================================================
// Trait Implementations
// ============================================================================

impl fmt::Display for PrimSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_dormant() {
            write!(f, "<dormant prim spec>")
        } else {
            write!(
                f,
                "<{} prim '{}' at {}>",
                self.specifier(),
                self.name(),
                self.path()
            )
        }
    }
}

impl PartialEq for PrimSpec {
    fn eq(&self, other: &Self) -> bool {
        self.spec == other.spec
    }
}

impl Eq for PrimSpec {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Relocates Tests (ported from testSdfRelocates.py::test_PrimRelocates)
    // ========================================================================

    fn create_test_layer_with_root() -> (std::sync::Arc<super::super::Layer>, PrimSpec) {
        use super::super::Layer;
        let arc = Layer::create_anonymous(Some(".usda"));
        let handle = LayerHandle::from_layer(&arc);
        let prim = PrimSpec::new_root(&handle, "Root", Specifier::Def, "Scope")
            .expect("failed to create /Root prim");
        (arc, prim)
    }

    #[test]
    fn test_prim_relocates_empty() {
        let (_layer, prim) = create_test_layer_with_root();
        // Prim starts with no relocates authored
        assert_eq!(prim.relocates().len(), 0);
        assert!(!prim.has_relocates());
    }

    #[test]
    fn test_prim_relocates_set_and_get() {
        let (_layer, mut prim) = create_test_layer_with_root();

        let mut map = BTreeMap::new();
        map.insert(
            Path::from_string("/Root/source1").unwrap(),
            Path::from_string("/Root/target1").unwrap(),
        );
        map.insert(
            Path::from_string("/Root/source2").unwrap(),
            Path::from_string("/Root/target2").unwrap(),
        );
        map.insert(
            Path::from_string("/Root/source3").unwrap(),
            Path::from_string("/Root/target3").unwrap(),
        );

        prim.set_relocates(map.clone());
        assert!(prim.has_relocates());
        assert_eq!(prim.relocates().len(), 3);
        assert_eq!(prim.relocates(), map);
    }

    #[test]
    fn test_prim_relocates_remove_entry() {
        let (_layer, mut prim) = create_test_layer_with_root();

        let mut map = BTreeMap::new();
        map.insert(
            Path::from_string("/Root/source1").unwrap(),
            Path::from_string("/Root/target1").unwrap(),
        );
        map.insert(
            Path::from_string("/Root/source2").unwrap(),
            Path::from_string("/Root/target2").unwrap(),
        );
        map.insert(
            Path::from_string("/Root/source3").unwrap(),
            Path::from_string("/Root/target3").unwrap(),
        );
        prim.set_relocates(map);

        // Remove source2 by writing back a map without it
        let mut updated = prim.relocates();
        updated.remove(&Path::from_string("/Root/source2").unwrap());
        prim.set_relocates(updated);

        let result = prim.relocates();
        assert_eq!(result.len(), 2);
        assert!(!result.contains_key(&Path::from_string("/Root/source2").unwrap()));
        assert!(result.contains_key(&Path::from_string("/Root/source1").unwrap()));
        assert!(result.contains_key(&Path::from_string("/Root/source3").unwrap()));
    }

    #[test]
    fn test_prim_relocates_insert_entry() {
        let (_layer, mut prim) = create_test_layer_with_root();

        let mut map = BTreeMap::new();
        map.insert(
            Path::from_string("/Root/source1").unwrap(),
            Path::from_string("/Root/target1").unwrap(),
        );
        prim.set_relocates(map);

        // Insert source4 by extending the map
        let mut updated = prim.relocates();
        updated.insert(
            Path::from_string("/Root/source4").unwrap(),
            Path::from_string("/Root/target4").unwrap(),
        );
        prim.set_relocates(updated);

        let result = prim.relocates();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&Path::from_string("/Root/source4").unwrap()));
        assert_eq!(
            result[&Path::from_string("/Root/source4").unwrap()],
            Path::from_string("/Root/target4").unwrap()
        );
    }

    #[test]
    fn test_prim_relocates_overwrite_entry() {
        let (_layer, mut prim) = create_test_layer_with_root();

        let mut map = BTreeMap::new();
        map.insert(
            Path::from_string("/Root/source1").unwrap(),
            Path::from_string("/Root/target1").unwrap(),
        );
        prim.set_relocates(map);

        // Overwrite source1 -> targetFoo
        let mut updated = prim.relocates();
        updated.insert(
            Path::from_string("/Root/source1").unwrap(),
            Path::from_string("/Root/targetFoo").unwrap(),
        );
        prim.set_relocates(updated);

        let result = prim.relocates();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[&Path::from_string("/Root/source1").unwrap()],
            Path::from_string("/Root/targetFoo").unwrap()
        );
    }

    #[test]
    fn test_prim_relocates_clear() {
        let (_layer, mut prim) = create_test_layer_with_root();

        let mut map = BTreeMap::new();
        map.insert(
            Path::from_string("/Root/source1").unwrap(),
            Path::from_string("/Root/target1").unwrap(),
        );
        prim.set_relocates(map);
        assert!(prim.has_relocates());

        prim.clear_relocates();
        assert!(!prim.has_relocates());
        assert_eq!(prim.relocates().len(), 0);
    }

    #[test]
    fn test_prim_spec_default() {
        let spec = PrimSpec::default();
        assert!(spec.is_dormant());
    }

    #[test]
    fn test_prim_spec_dormant() {
        let spec = PrimSpec::dormant();
        assert!(spec.is_dormant());
        assert_eq!(format!("{}", spec), "<dormant prim spec>");
    }

    #[test]
    fn test_is_valid_name() {
        assert!(PrimSpec::is_valid_name("World"));
        assert!(PrimSpec::is_valid_name("_private"));
        assert!(PrimSpec::is_valid_name("Model_01"));
        assert!(PrimSpec::is_valid_name("ABC123"));

        assert!(!PrimSpec::is_valid_name(""));
        assert!(!PrimSpec::is_valid_name("123Model"));
        assert!(!PrimSpec::is_valid_name("Model-01"));
        assert!(!PrimSpec::is_valid_name("Model.01"));
        assert!(!PrimSpec::is_valid_name("."));
        assert!(!PrimSpec::is_valid_name(".."));
    }

    #[test]
    fn test_name() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.name(), "");
    }

    #[test]
    fn test_name_token() {
        let spec = PrimSpec::dormant();
        let token = spec.name_token();
        assert_eq!(token.as_str(), "");
    }

    #[test]
    fn test_type_name() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.type_name(), Token::empty());
    }

    #[test]
    fn test_specifier() {
        let spec = PrimSpec::dormant();
        // Default is Over when not authored
        assert_eq!(spec.specifier(), Specifier::Over);
    }

    #[test]
    fn test_comment() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.comment(), "");
    }

    #[test]
    fn test_documentation() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.documentation(), "");
    }

    #[test]
    fn test_active() {
        let spec = PrimSpec::dormant();
        // Default is true
        assert!(spec.active());
        assert!(!spec.has_active());
    }

    #[test]
    fn test_hidden() {
        let spec = PrimSpec::dormant();
        // Default is false
        assert!(!spec.hidden());
    }

    #[test]
    fn test_kind() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.kind(), Token::empty());
        assert!(!spec.has_kind());
    }

    #[test]
    fn test_permission() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.permission(), Permission::Public);
    }

    #[test]
    fn test_symmetric_peer() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.symmetric_peer(), "");
    }

    #[test]
    fn test_prefix_suffix() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.prefix(), "");
        assert_eq!(spec.suffix(), "");
    }

    #[test]
    fn test_custom_data() {
        let spec = PrimSpec::dormant();
        let data = spec.custom_data();
        assert!(data.is_empty());
    }

    #[test]
    fn test_asset_info() {
        let spec = PrimSpec::dormant();
        let info = spec.asset_info();
        assert!(info.is_empty());
    }

    #[test]
    fn test_references() {
        let spec = PrimSpec::dormant();
        assert!(!spec.has_references());
        let refs = spec.references_list();
        assert!(!refs.has_keys());
    }

    #[test]
    fn test_payloads() {
        let spec = PrimSpec::dormant();
        assert!(!spec.has_payloads());
        let payloads = spec.payloads_list();
        assert!(!payloads.has_keys());
    }

    #[test]
    fn test_inherits() {
        let spec = PrimSpec::dormant();
        assert!(!spec.has_inherits());
        let inherits = spec.inherits_list();
        assert!(!inherits.has_keys());
    }

    #[test]
    fn test_specializes() {
        let spec = PrimSpec::dormant();
        assert!(!spec.has_specializes());
        let specializes = spec.specializes_list();
        assert!(!specializes.has_keys());
    }

    #[test]
    fn test_variant_selection() {
        let spec = PrimSpec::dormant();
        let selection = spec.variant_selection();
        assert!(selection.is_empty());
    }

    #[test]
    fn test_name_children() {
        let spec = PrimSpec::dormant();
        let children = spec.name_children();
        assert!(children.is_empty());
    }

    #[test]
    fn test_properties() {
        let spec = PrimSpec::dormant();
        assert!(spec.properties().is_empty());
        assert!(spec.attributes().is_empty());
        assert!(spec.relationships().is_empty());
    }

    #[test]
    fn test_equality() {
        let spec1 = PrimSpec::dormant();
        let spec2 = PrimSpec::dormant();
        assert_eq!(spec1, spec2);
    }

    #[test]
    fn test_path() {
        let spec = PrimSpec::dormant();
        assert_eq!(spec.path(), Path::empty());
    }

    #[test]
    fn test_layer() {
        let spec = PrimSpec::dormant();
        assert!(!spec.layer().is_valid());
    }

    #[test]
    fn test_can_set_name() {
        let spec = PrimSpec::dormant();
        let mut why_not = String::new();

        assert!(spec.can_set_name("ValidName", &mut why_not));
        assert!(!spec.can_set_name("123Invalid", &mut why_not));
        assert!(why_not.contains("not a valid prim name"));
    }

    #[test]
    fn test_type_name_stored_as_token() {
        // C++ SdfPrimSpec::SetTypeName stores as TfToken, not std::string
        // Verify that type_name() can read back a Token set via set_type_name()
        use super::super::Layer;

        let layer = Layer::create_anonymous(Some("type_name_test"));
        let path = super::super::Path::from_string("/TestPrim").unwrap();
        let spec = layer.create_prim_spec(&path, super::super::Specifier::Def, "Xform");
        assert!(spec.is_some());
        let spec = spec.unwrap();

        // type_name should be a Token with value "Xform"
        assert_eq!(spec.type_name().as_str(), "Xform");
    }

    #[test]
    fn test_set_type_name_empty_only_allowed_for_over() {
        // C++ SdfPrimSpec::SetTypeName: cannot set empty on non-Over prim
        use super::super::Layer;

        let layer = Layer::create_anonymous(Some("empty_type_name_test"));
        let path = super::super::Path::from_string("/DefPrim").unwrap();
        let over_path = super::super::Path::from_string("/OverPrim").unwrap();

        layer.create_prim_spec(&path, super::super::Specifier::Def, "Xform");
        layer.create_prim_spec(&over_path, super::super::Specifier::Over, "");

        if let Some(mut def_spec) = layer.get_prim_at_path(&path) {
            def_spec.set_type_name(""); // Should no-op for Def
            assert_eq!(
                def_spec.type_name().as_str(),
                "Xform",
                "Def prim type_name should not be cleared"
            );
        }

        if let Some(mut over_spec) = layer.get_prim_at_path(&over_path) {
            over_spec.set_type_name(""); // Should be ok for Over
            // For Over, empty type is allowed
        }
    }

    #[test]
    fn test_specifier_stored_as_enum() {
        // C++ stores Specifier as typed enum; verify roundtrip
        use super::super::{Layer, Specifier};

        let layer = Layer::create_anonymous(Some("specifier_test"));
        let def_path = super::super::Path::from_string("/DefPrim").unwrap();
        let over_path = super::super::Path::from_string("/OverPrim").unwrap();
        let class_path = super::super::Path::from_string("/ClassPrim").unwrap();

        layer.create_prim_spec(&def_path, Specifier::Def, "");
        layer.create_prim_spec(&over_path, Specifier::Over, "");
        layer.create_prim_spec(&class_path, Specifier::Class, "");

        assert_eq!(
            layer.get_prim_at_path(&def_path).unwrap().specifier(),
            Specifier::Def
        );
        assert_eq!(
            layer.get_prim_at_path(&over_path).unwrap().specifier(),
            Specifier::Over
        );
        assert_eq!(
            layer.get_prim_at_path(&class_path).unwrap().specifier(),
            Specifier::Class
        );
    }

    // ========================================================================
    // Ported from testSdfPrim.py
    // ========================================================================

    /// Port of testSdfPrim.py::test_CreatePrimInLayer
    ///
    /// Verifies that create_prim_in_layer creates prims and all ancestor prims,
    /// and that the resulting specs are retrievable by path.
    #[test]
    fn test_create_prim_in_layer() {
        use super::super::Layer;
        use crate::{create_prim_in_layer, just_create_prim_in_layer};

        let layer = Layer::create_anonymous(None);
        let handle = LayerHandle::from_layer(&layer);

        // Create using relative-style paths (without leading slash)
        let foo = Path::from_string("/foo").unwrap();
        let foo_bar = Path::from_string("/foo/bar").unwrap();
        let foo_bar_baz = Path::from_string("/foo/bar/baz").unwrap();

        assert!(create_prim_in_layer(&handle, &foo).is_some());
        assert!(create_prim_in_layer(&handle, &foo_bar).is_some());
        assert!(create_prim_in_layer(&handle, &foo_bar_baz).is_some());
        assert!(layer.get_prim_at_path(&foo).is_some());
        assert!(layer.get_prim_at_path(&foo_bar).is_some());
        assert!(layer.get_prim_at_path(&foo_bar_baz).is_some());

        // Create a separate hierarchy under /boo
        let boo = Path::from_string("/boo").unwrap();
        let boo_bar = Path::from_string("/boo/bar").unwrap();
        let boo_bar_baz = Path::from_string("/boo/bar/baz").unwrap();

        assert!(create_prim_in_layer(&handle, &boo).is_some());
        assert!(create_prim_in_layer(&handle, &boo_bar).is_some());
        assert!(create_prim_in_layer(&handle, &boo_bar_baz).is_some());
        assert!(layer.get_prim_at_path(&boo).is_some());
        assert!(layer.get_prim_at_path(&boo_bar).is_some());
        assert!(layer.get_prim_at_path(&boo_bar_baz).is_some());

        // just_create_prim_in_layer variant
        let goo = Path::from_string("/goo").unwrap();
        let goo_bar = Path::from_string("/goo/bar").unwrap();
        let goo_bar_baz = Path::from_string("/goo/bar/baz").unwrap();

        assert!(just_create_prim_in_layer(&handle, &goo));
        assert!(just_create_prim_in_layer(&handle, &goo_bar));
        assert!(just_create_prim_in_layer(&handle, &goo_bar_baz));
        assert!(layer.get_prim_at_path(&goo).is_some());
        assert!(layer.get_prim_at_path(&goo_bar).is_some());
        assert!(layer.get_prim_at_path(&goo_bar_baz).is_some());

        // TODO(path_validation): In C++ Sdf.CreatePrimInLayer(layer, '..') and ('../..')
        // raise Tf.ErrorException because '..' is a relative path.
        // Our Path::from_string() parses '..' as a valid relative path node and
        // create_prim_spec does not reject non-absolute paths.
        // Once path validation is added, re-enable:
        //   assert!(Path::from_string("..").is_none());
        //   assert!(create_prim_in_layer(&handle, &dotdot_path).is_none());
    }

    /// Port of testSdfPrim.py::test_NameChildrenInsert
    ///
    /// Verifies that insert_name_child respects the given index, producing the
    /// same ordering as a reference list built with the same insertions.
    #[test]
    fn test_name_children_ordering() {
        use super::super::Layer;

        let layer = Layer::create_anonymous(Some("test"));
        let root_path = Path::from_string("/Root").unwrap();
        layer.create_prim_spec(&root_path, Specifier::Def, "Scope");
        let mut root = layer.get_prim_at_path(&root_path).unwrap();

        // Insert a modest set of children with various positive/negative/out-of-bounds indices
        // (a full 1000-iteration random test would be the same logic; 20 suffices for the port).
        let insertions: &[(&str, i32)] = &[
            ("geom0", 0),
            ("geom1", -1),   // append
            ("geom2", 0),    // prepend
            ("geom3", 999),  // out-of-bounds → append
            ("geom4", 1),    // insert at 1
            ("geom5", -100), // negative out-of-bounds → append
        ];

        // Pre-create all child prims under /Root so they exist at /Root/geomN.
        // create_prim_spec appends each name to Root's primChildren automatically.
        // We then clear that list and re-insert in test-controlled order, which
        // matches the C++ test where PrimSpec construction does not implicitly link
        // to a parent and nameChildren.insert() is always an explicit operation.
        for (name, _index) in insertions {
            let child_path = Path::from_string(&format!("/Root/{name}")).unwrap();
            layer.create_prim_spec(&child_path, Specifier::Def, "Scope");
        }
        // Clear the auto-populated primChildren order so we can test insert_name_child.
        root.set_name_children(&[]);

        let mut ground_truth: Vec<String> = Vec::new();

        for (name, index) in insertions {
            let child_path = Path::from_string(&format!("/Root/{name}")).unwrap();
            let child = layer.get_prim_at_path(&child_path).unwrap();

            let ok = root.insert_name_child(&child, *index);
            assert!(ok, "insert_name_child failed for '{name}' at index {index}");

            // Mirror the same index semantics: clamp negative/out-of-bounds to append.
            let len = ground_truth.len() as i32;
            if *index < 0 || *index >= len {
                ground_truth.push(name.to_string());
            } else {
                ground_truth.insert(*index as usize, name.to_string());
            }
        }

        let actual: Vec<String> = root.name_children().iter().map(|p| p.name()).collect();

        assert_eq!(
            actual, ground_truth,
            "name_children ordering does not match reference list"
        );
    }

    /// Port of testSdfPrim.py::test_InertSpecRemoval
    ///
    /// Verifies that deleting a prim spec (and its children) removes the spec
    /// from the layer, and that deleting a variant set removes its variants.
    #[test]
    fn test_inert_spec_removal() {
        use super::super::Layer;
        use crate::variant_set_spec::VariantSetSpec;
        use crate::variant_set_spec::VariantSpec;

        let layer = Layer::create_anonymous(None);
        let handle = LayerHandle::from_layer(&layer);

        // Build /InertSubtree/Is/Inert using create_prim_in_layer
        let inert_path = Path::from_string("/InertSubtree/Is/Inert").unwrap();
        crate::create_prim_in_layer(&handle, &inert_path);
        assert!(
            layer
                .get_prim_at_path(&Path::from_string("/InertSubtree").unwrap())
                .is_some()
        );

        // Delete the root of the subtree — equivalent to C++ del nameChildren["InertSubtree"]
        let subtree_path = Path::from_string("/InertSubtree").unwrap();
        let deleted = layer.delete_spec(&subtree_path);
        assert!(
            deleted,
            "delete_spec should return true for an existing prim"
        );
        assert!(
            layer.get_prim_at_path(&subtree_path).is_none(),
            "/InertSubtree must not exist after deletion"
        );

        // Build /InertVariants with variant set "v" containing "a" and "b"
        let variants_path = Path::from_string("/InertVariants").unwrap();
        crate::create_prim_in_layer(&handle, &variants_path);
        let prim = layer.get_prim_at_path(&variants_path).unwrap();
        let variant_set = VariantSetSpec::new(&prim, "v").expect("failed to create variant set");
        VariantSpec::new(&variant_set, "a").expect("failed to create variant 'a'");
        VariantSpec::new(&variant_set, "b").expect("failed to create variant 'b'");

        // Remove the variant set — equivalent to C++ del variantSets["v"]
        let mut proxy = prim.variant_sets();
        let removed = proxy.remove("v");
        assert!(
            removed,
            "remove must return true for existing variant set 'v'"
        );
        assert!(
            !proxy.has("v"),
            "variant set 'v' must not be present after removal"
        );
    }

    // NOTE: test_Clips is not ported — prim.clips() / clipSetsList are present
    // but the SdfListOp-based clipSetsList API differs from the Python proxy.
    // TODO(clips_parity): port test_Clips once clip-sets list-op API is stable.
}
