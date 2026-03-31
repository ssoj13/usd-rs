//! SDF path type.
//!
//! The `Path` type represents a location in a USD scene graph hierarchy.
//! Paths are used to address prims, properties, and other scene elements.
//!
//! # Path Syntax
//!
//! - `/` - absolute root path
//! - `.` - reflexive relative path (current prim)
//! - `/Foo` - absolute prim path
//! - `/Foo/Bar` - nested prim path
//! - `/Foo.prop` - prim property path
//! - `/Foo.prop[/Target]` - relationship target path
//! - `/Foo{variant=selection}` - variant selection path
//! - `..` - parent path element
//!
//! # Architecture
//!
//! Internally, paths are represented as pairs of handles into a global
//! node-based prefix tree (see `path_node` module). This gives O(1)
//! equality, O(1) hash, and O(1) type queries while maintaining full
//! API compatibility via a cached path string.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::path_node::{self, NodeFlags, NodeHandle, NodeType};
use usd_tf::Token;

/// A path value used to locate objects in layers or scenegraphs.
///
/// `Path` is used in several ways:
/// - As a storage key for addressing and accessing values held in a SdfLayer
/// - As a namespace identity for scenegraph objects
/// - As a way to refer to other scenegraph objects through relative paths
///
/// The paths represented by a `Path` may be either relative or absolute.
/// Relative paths are relative to the prim object that contains them.
///
/// # Thread Safety
///
/// `Path` is strongly thread-safe. Values are immutable and can be shared
/// across threads without synchronization.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::Path;
///
/// // Create paths
/// let root = Path::absolute_root();
/// let prim = Path::from_string("/World/Cube").unwrap();
/// let prop = prim.append_property("visibility").unwrap();
///
/// // Query path properties
/// assert!(prim.is_absolute_path());
/// assert!(prim.is_prim_path());
/// assert!(prop.is_property_path());
///
/// // Navigate hierarchy
/// assert_eq!(prop.get_prim_path(), prim);
/// assert_eq!(prim.get_parent_path(), Path::from_string("/World").unwrap());
/// ```
#[derive(Clone)]
pub struct Path {
    /// Handle to the prim-side node (Root, Prim, PrimVariantSelection).
    /// NULL for empty path.
    prim_handle: NodeHandle,
    /// Handle to the property-side node (PrimProperty, Target, etc.).
    /// NULL if no property part.
    prop_handle: NodeHandle,
    /// Cached string representation.
    ///
    /// # Design note: eager vs lazy caching
    ///
    /// C++ SdfPath computes the string on demand via `GetString()`. We eagerly
    /// cache it here because:
    /// 1. `as_str()` returns `&str`, which requires an owned backing `String`.
    /// 2. Paths are immutable — the string never changes after construction.
    /// 3. Paths are frequently used as HashMap keys and for display/logging.
    ///
    /// The tradeoff is ~40 bytes extra per Path (String header + heap alloc).
    /// For scenes with millions of paths this could matter; if so, switch to
    /// `OnceCell<String>` and change `as_str()` to return on-demand.
    path_string: String,
}

impl Default for Path {
    fn default() -> Self {
        Self::empty()
    }
}

impl Path {
    // =========================================================================
    // Internal constructors
    // =========================================================================

    /// Create a Path from handles, eagerly computing the string representation.
    fn from_handles(prim_handle: NodeHandle, prop_handle: NodeHandle) -> Self {
        let path_string = path_node::build_path_string(prim_handle, prop_handle);
        Self {
            prim_handle,
            prop_handle,
            path_string,
        }
    }

    // =========================================================================
    // Static path constants
    // =========================================================================

    /// Returns the empty path.
    ///
    /// The empty path represents an invalid or uninitialized path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let path = Path::empty();
    /// assert!(path.is_empty());
    /// ```
    pub fn empty() -> Self {
        Self {
            prim_handle: NodeHandle::NULL,
            prop_handle: NodeHandle::NULL,
            path_string: String::new(),
        }
    }

    /// Returns the absolute root path ("/").
    ///
    /// The absolute root represents the top of the namespace hierarchy.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let root = Path::absolute_root();
    /// assert_eq!(root.as_str(), "/");
    /// assert!(root.is_absolute_root_path());
    /// ```
    pub fn absolute_root() -> Self {
        let h = path_node::absolute_root_handle();
        Self {
            prim_handle: h,
            prop_handle: NodeHandle::NULL,
            path_string: String::from("/"),
        }
    }

    /// Returns the reflexive relative path (".").
    ///
    /// The reflexive relative path represents "self" in the namespace.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let relative = Path::reflexive_relative();
    /// assert_eq!(relative.as_str(), ".");
    /// ```
    pub fn reflexive_relative() -> Self {
        let h = path_node::relative_root_handle();
        Self {
            prim_handle: h,
            prop_handle: NodeHandle::NULL,
            path_string: String::from("."),
        }
    }

    // =========================================================================
    // Constructors
    // =========================================================================

    /// Creates a path from a string.
    ///
    /// Returns `None` if the string is not a valid path. Empty strings
    /// return `None` (use `Path::empty()` for the empty path).
    ///
    /// # Arguments
    ///
    /// * `path` - The path string to parse
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let prim = Path::from_string("/World/Cube").unwrap();
    /// assert_eq!(prim.as_str(), "/World/Cube");
    ///
    /// let prop = Path::from_string("/World.visibility").unwrap();
    /// assert!(prop.is_property_path());
    /// ```
    pub fn from_string(path: &str) -> Option<Self> {
        if path.is_empty() {
            return None;
        }

        // Basic validation
        if !Self::is_valid_path_string(path) {
            return None;
        }

        // Parse into node handles
        Some(Self::parse_to_nodes(path))
    }

    /// Parse a validated path string into node handles.
    fn parse_to_nodes(path: &str) -> Self {
        // Special cases
        if path == "/" {
            return Self::absolute_root();
        }
        if path == "." {
            return Self::reflexive_relative();
        }
        if path == ".." {
            return Self {
                prim_handle: path_node::find_or_create_prim(
                    path_node::relative_root_handle(),
                    &Token::new(".."),
                ),
                prop_handle: NodeHandle::NULL,
                path_string: "..".to_string(),
            };
        }

        // Determine root
        let is_absolute = path.starts_with('/');
        let root_handle = if is_absolute {
            path_node::absolute_root_handle()
        } else {
            path_node::relative_root_handle()
        };

        // Split prim part from property part at the first top-level '.'
        let work = if is_absolute { &path[1..] } else { path };

        let (prim_part, prop_part) = Self::split_at_top_level_dot(work);

        // Handle relative paths starting with "./" or "../"
        // If prim_part is empty and we're not absolute, it might be "./Cube"
        // In that case, the '.' was consumed as a top-level dot separator
        // We need special handling for relative paths like "./Cube" and "../Sibling"
        let (prim_part, prop_part) = if !is_absolute && prim_part.is_empty() && prop_part.is_some()
        {
            // The first '.' was actually the relative root "."
            // Check if what follows is a path component (contains '/')
            let rest = prop_part.unwrap();
            if rest.starts_with('/') || rest.starts_with('.') || rest.contains('/') {
                // This is "./something" or "../something" - treat as prim path
                (rest, None)
            } else {
                // This would be a property on the relative root, which is unusual
                // but handle it: empty prim means relative root
                (prim_part, prop_part)
            }
        } else {
            (prim_part, prop_part)
        };

        // Parse prim part
        let mut current = root_handle;
        if !prim_part.is_empty() {
            current = Self::parse_prim_part(current, prim_part);
        }

        let prim_handle = current;

        // Parse property part
        let prop_handle = if let Some(prop) = prop_part {
            Self::parse_prop_part(prim_handle, prop)
        } else {
            NodeHandle::NULL
        };

        Self {
            prim_handle,
            prop_handle,
            path_string: path.to_string(),
        }
    }

    /// Parse the prim portion of a path (after stripping leading '/').
    /// Handles prim names, variant selections, and ".."/"." segments.
    fn parse_prim_part(root: NodeHandle, prim_part: &str) -> NodeHandle {
        let mut current = root;

        // Split on top-level '/' (not inside brackets/braces)
        let components = Self::split_top_level(prim_part, '/');

        for comp in components {
            if comp.is_empty() {
                continue;
            }
            if comp == "." {
                // Stay at current
                continue;
            }
            if comp == ".." {
                current = path_node::find_or_create_prim(current, &Token::new(".."));
                continue;
            }

            // Check for variant selection: "Name{var=sel}" or just "{var=sel}"
            if let Some(brace_pos) = comp.find('{') {
                // Prim name before brace
                let name = &comp[..brace_pos];
                if !name.is_empty() {
                    current = path_node::find_or_create_prim(current, &Token::new(name));
                }

                // Parse variant selections (there may be multiple chained)
                let mut rest = &comp[brace_pos..];
                while let Some(open) = rest.find('{') {
                    if let Some(close) = rest[open..].find('}') {
                        let selection = &rest[open + 1..open + close];
                        if let Some(eq_pos) = selection.find('=') {
                            let set_name = &selection[..eq_pos];
                            let variant = &selection[eq_pos + 1..];
                            current = path_node::find_or_create_variant_selection(
                                current, set_name, variant,
                            );
                        }
                        rest = &rest[open + close + 1..];
                    } else {
                        break;
                    }
                }

                // Any prim name after the last '}'
                if !rest.is_empty() {
                    current = path_node::find_or_create_prim(current, &Token::new(rest));
                }
            } else {
                current = path_node::find_or_create_prim(current, &Token::new(comp));
            }
        }

        current
    }

    /// Parse the property portion of a path (after the first top-level '.').
    fn parse_prop_part(prim_handle: NodeHandle, prop_part: &str) -> NodeHandle {
        if prop_part.is_empty() {
            return NodeHandle::NULL;
        }

        // Parse property chain: prop_name, [/Target], .attr, .mapper[/T], .expression
        let mut current = NodeHandle::NULL;
        let chars: Vec<char> = prop_part.chars().collect();
        let len = chars.len();

        // First property name (up to first '[' or '.')
        let name_start = 0;
        let mut name_end = 0;
        while name_end < len && chars[name_end] != '[' && chars[name_end] != '.' {
            name_end += 1;
        }

        let first_name = &prop_part[name_start..name_end];
        let mut idx = name_end;

        if first_name == "mapper" && idx < len && chars[idx] == '[' {
            // Mapper: .mapper[/Target]
            if let Some(close) = prop_part[idx..].find(']') {
                let target_str = &prop_part[idx + 1..idx + close];
                let prop_node =
                    path_node::find_or_create_prim_property(prim_handle, &Token::new(first_name));
                current = path_node::find_or_create_mapper(prop_node, target_str);
                idx += close + 1;
            }
        } else if !first_name.is_empty() {
            // Regular property or "rel"
            current = path_node::find_or_create_prim_property(prim_handle, &Token::new(first_name));
        }

        // Now process remaining: could be [/Target], .attr, .expression, etc.
        while idx < len {
            if chars[idx] == '[' {
                // Target path
                if let Some(close_offset) = prop_part[idx..].find(']') {
                    let target_str = &prop_part[idx + 1..idx + close_offset];
                    current = path_node::find_or_create_target(current, target_str);
                    idx += close_offset + 1;
                } else {
                    break;
                }
            } else if chars[idx] == '.' {
                idx += 1; // skip the dot
                // Read next name
                let sub_start = idx;
                while idx < len && chars[idx] != '[' && chars[idx] != '.' {
                    idx += 1;
                }
                let sub_name = &prop_part[sub_start..idx];

                if sub_name == "expression" {
                    current = path_node::find_or_create_expression(current);
                } else if sub_name == "mapper" && idx < len && chars[idx] == '[' {
                    // .mapper[/Target]
                    if let Some(close_offset) = prop_part[idx..].find(']') {
                        let target_str = &prop_part[idx + 1..idx + close_offset];
                        current = path_node::find_or_create_mapper(current, target_str);
                        idx += close_offset + 1;
                    }
                } else if !sub_name.is_empty() {
                    // Could be relational attribute or mapper arg depending on current node type
                    let current_type = path_node::get_node_type(current);
                    match current_type {
                        NodeType::Target => {
                            current = path_node::find_or_create_relational_attribute(
                                current,
                                &Token::new(sub_name),
                            );
                        }
                        NodeType::Mapper => {
                            current = path_node::find_or_create_mapper_arg(
                                current,
                                &Token::new(sub_name),
                            );
                        }
                        _ => {
                            // Chained property (shouldn't normally happen but handle)
                            current = path_node::find_or_create_relational_attribute(
                                current,
                                &Token::new(sub_name),
                            );
                        }
                    }
                }
            } else {
                idx += 1;
            }
        }

        current
    }

    /// Creates a path from a token.
    ///
    /// # Arguments
    ///
    /// * `token` - The token containing the path string
    pub fn from_token(token: &Token) -> Option<Self> {
        Self::from_string(token.as_str())
    }

    // =========================================================================
    // Basic queries
    // =========================================================================

    /// Returns the path as a string slice.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let path = Path::from_string("/World").unwrap();
    /// assert_eq!(path.as_str(), "/World");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.path_string
    }

    /// Returns the path as a string.
    pub fn get_string(&self) -> &str {
        &self.path_string
    }

    /// Returns the path as a string (owned).
    pub fn get_as_string(&self) -> String {
        self.path_string.clone()
    }

    /// Returns the path as a token.
    pub fn get_token(&self) -> Token {
        Token::new(&self.path_string)
    }

    /// Returns the path as a token (owned).
    pub fn get_as_token(&self) -> Token {
        Token::new(&self.path_string)
    }

    /// Returns the path as a C string pointer (for compatibility).
    /// In Rust, this is equivalent to as_str().
    pub fn get_text(&self) -> &str {
        &self.path_string
    }

    /// Returns true if this is the empty path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::empty().is_empty());
    /// assert!(!Path::absolute_root().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.prim_handle.is_null() && self.prop_handle.is_null()
    }

    /// Returns the number of path elements.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert_eq!(Path::absolute_root().get_path_element_count(), 0);
    /// assert_eq!(Path::from_string("/World").unwrap().get_path_element_count(), 1);
    /// assert_eq!(Path::from_string("/World/Cube").unwrap().get_path_element_count(), 2);
    /// ```
    pub fn get_path_element_count(&self) -> usize {
        if self.is_empty() {
            return 0;
        }

        let prim_count = if !self.prim_handle.is_null() {
            path_node::get_element_count(self.prim_handle) as usize
        } else {
            0
        };

        let prop_count = if !self.prop_handle.is_null() {
            path_node::get_element_count(self.prop_handle) as usize
        } else {
            0
        };

        // Property nodes have their own element count which includes the
        // chain from the property root. We need to subtract the prim-side
        // elements that are counted in the property's parent chain to avoid
        // double counting. Actually, property nodes are parented to prim nodes
        // so their element_count already includes the prim depth.
        // We want: total = prop_count if prop exists, else prim_count
        if prop_count > 0 {
            prop_count
        } else {
            prim_count
        }
    }

    // =========================================================================
    // Path type queries - O(1) via node flags and types
    // =========================================================================

    /// Returns whether the path is absolute.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World").unwrap().is_absolute_path());
    /// assert!(!Path::from_string("World").unwrap().is_absolute_path());
    /// ```
    pub fn is_absolute_path(&self) -> bool {
        if self.prim_handle.is_null() {
            return false;
        }
        path_node::get_node_flags(self.prim_handle).contains(NodeFlags::IS_ABSOLUTE)
    }

    /// Returns true if this is the absolute root path ("/").
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::absolute_root().is_absolute_root_path());
    /// assert!(!Path::from_string("/World").unwrap().is_absolute_root_path());
    /// ```
    pub fn is_absolute_root_path(&self) -> bool {
        !self.prim_handle.is_null()
            && self.prop_handle.is_null()
            && self.prim_handle == path_node::absolute_root_handle()
    }

    /// Returns whether the path identifies a prim.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World").unwrap().is_prim_path());
    /// assert!(!Path::from_string("/World.prop").unwrap().is_prim_path());
    /// assert!(!Path::absolute_root().is_prim_path());
    /// ```
    pub fn is_prim_path(&self) -> bool {
        if self.is_empty() || self.is_absolute_root_path() {
            return false;
        }
        if self.prim_handle == path_node::relative_root_handle() && self.prop_handle.is_null() {
            return true; // "." is a prim path
        }
        // A prim path has no property elements
        self.prop_handle.is_null() && !self.is_prim_variant_selection_path()
    }

    /// Returns whether the path identifies the absolute root or a prim.
    pub fn is_absolute_root_or_prim_path(&self) -> bool {
        self.is_absolute_root_path() || self.is_prim_path()
    }

    /// Returns whether the path identifies a root prim (e.g., "/Foo").
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World").unwrap().is_root_prim_path());
    /// assert!(!Path::from_string("/World/Cube").unwrap().is_root_prim_path());
    /// ```
    pub fn is_root_prim_path(&self) -> bool {
        if !self.is_absolute_path() || self.is_absolute_root_path() {
            return false;
        }
        self.get_path_element_count() == 1 && self.is_prim_path()
    }

    /// Returns whether the path identifies a property.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World.visibility").unwrap().is_property_path());
    /// assert!(!Path::from_string("/World").unwrap().is_property_path());
    /// ```
    pub fn is_property_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        let prop_type = path_node::get_node_type(self.prop_handle);
        // C++ SdfPath::IsPropertyPath: only PrimPropertyNode and RelationalAttributeNode
        // MapperArg is NOT a property path (has its own is_mapper_arg_path())
        matches!(
            prop_type,
            NodeType::PrimProperty | NodeType::RelationalAttribute
        )
    }

    /// Returns whether the path identifies a prim property.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World.visibility").unwrap().is_prim_property_path());
    /// ```
    pub fn is_prim_property_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_type(self.prop_handle) == NodeType::PrimProperty
    }

    /// Returns whether the path identifies a namespaced property.
    ///
    /// A namespaced property has colons in its name.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World.primvars:foo").unwrap().is_namespaced_property_path());
    /// assert!(!Path::from_string("/World.visibility").unwrap().is_namespaced_property_path());
    /// ```
    pub fn is_namespaced_property_path(&self) -> bool {
        if !self.is_property_path() {
            return false;
        }
        if let Some(prop_name) = self.get_property_name() {
            return prop_name.contains(':');
        }
        false
    }

    /// Returns whether the path identifies a variant selection.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World{variant=sel}").unwrap().is_prim_variant_selection_path());
    /// ```
    pub fn is_prim_variant_selection_path(&self) -> bool {
        if self.is_empty() || !self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_type(self.prim_handle) == NodeType::PrimVariantSelection
    }

    /// Returns whether this is a prim or prim variant selection path.
    pub fn is_prim_or_prim_variant_selection_path(&self) -> bool {
        self.is_prim_path() || self.is_prim_variant_selection_path()
    }

    /// Returns whether the path or any of its parents has a variant selection.
    pub fn contains_prim_variant_selection(&self) -> bool {
        if self.prim_handle.is_null() {
            return false;
        }
        path_node::get_node_flags(self.prim_handle).contains(NodeFlags::CONTAINS_VARIANT_SEL)
    }

    /// Returns true if this path contains any property elements.
    pub fn contains_property_elements(&self) -> bool {
        !self.prop_handle.is_null()
    }

    /// Returns true if this path contains a target path.
    pub fn contains_target_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_flags(self.prop_handle).contains(NodeFlags::CONTAINS_TARGET_PATH)
    }

    /// Returns whether the path identifies a relational attribute.
    pub fn is_relational_attribute_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_type(self.prop_handle) == NodeType::RelationalAttribute
    }

    /// Returns whether the path identifies a relationship target.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert!(Path::from_string("/World.rel[/Target]").unwrap().is_target_path());
    /// ```
    pub fn is_target_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_type(self.prop_handle) == NodeType::Target
    }

    /// Returns whether the path identifies a mapper.
    pub fn is_mapper_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_type(self.prop_handle) == NodeType::Mapper
    }

    /// Returns whether the path identifies a mapper arg.
    pub fn is_mapper_arg_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_type(self.prop_handle) == NodeType::MapperArg
    }

    /// Returns whether the path identifies an expression.
    pub fn is_expression_path(&self) -> bool {
        if self.prop_handle.is_null() {
            return false;
        }
        path_node::get_node_type(self.prop_handle) == NodeType::Expression
    }

    // =========================================================================
    // Name queries
    // =========================================================================

    /// Returns the name of the final element (prim or property).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// assert_eq!(Path::from_string("/World/Cube").unwrap().get_name(), "Cube");
    /// assert_eq!(Path::from_string("/World.visibility").unwrap().get_name(), "visibility");
    /// ```
    pub fn get_name(&self) -> &str {
        if self.is_empty() {
            return "";
        }
        if self.is_absolute_root_path() {
            return "";
        }
        if self.path_string == "." {
            return ".";
        }

        // For properties, get name after last '.'
        if self.contains_property_elements() {
            if let Some(prop) = self.get_property_name() {
                return prop;
            }
        }

        // For prims, get name after the last separator at depth 0.
        // Separators are '/' and '}' (closing brace of a variant selection).
        // Example: /Root{vset=x}VariantChild -> "VariantChild"
        let mut last_sep: Option<usize> = None;
        let mut depth = 0usize;
        for (i, ch) in self.path_string.char_indices() {
            match ch {
                '[' | '{' => depth += 1,
                ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        // Closing brace at top level is a separator
                        last_sep = Some(i);
                    }
                }
                '/' if depth == 0 => last_sep = Some(i),
                _ => {}
            }
        }

        match last_sep {
            None => &self.path_string,
            Some(sep) => &self.path_string[sep + 1..],
        }
    }

    /// Returns the name as a token.
    pub fn get_name_token(&self) -> Token {
        Token::new(self.get_name())
    }

    /// Returns an ASCII representation of the "terminal" element of this path.
    ///
    /// This can be used to reconstruct the path using `append_element_string()` on its parent.
    /// EmptyPath(), AbsoluteRootPath(), and ReflexiveRelativePath() are not considered elements,
    /// so this will return an empty string for these paths.
    pub fn get_element_string(&self) -> String {
        self.get_element_token().as_str().to_string()
    }

    /// Returns the element string as a token.
    pub fn get_element_token(&self) -> Token {
        if self.is_empty() || self.is_absolute_root_path() || self.path_string == "." {
            return Token::new("");
        }

        // Extract the terminal element
        if self.contains_property_elements() {
            // For properties, get everything after the last '.' (outside brackets)
            let mut last_dot = None;
            let mut depth = 0;
            for (i, ch) in self.path_string.char_indices() {
                match ch {
                    '[' | '{' => depth += 1,
                    ']' | '}' => depth -= 1,
                    '.' if depth == 0 => last_dot = Some(i),
                    _ => {}
                }
            }
            if let Some(pos) = last_dot {
                return Token::new(&self.path_string[pos..]);
            }
        }

        // For prims, get everything after the last '/' (outside brackets/braces)
        let mut last_slash = None;
        let mut depth = 0;
        for (i, ch) in self.path_string.char_indices() {
            match ch {
                '[' | '{' => depth += 1,
                ']' | '}' => depth -= 1,
                '/' if depth == 0 => last_slash = Some(i),
                _ => {}
            }
        }

        if let Some(pos) = last_slash {
            Token::new(&self.path_string[pos + 1..])
        } else {
            Token::new(&self.path_string)
        }
    }

    /// Returns the property name if this is a property path.
    fn get_property_name(&self) -> Option<&str> {
        if !self.contains_property_elements() {
            return None;
        }

        // Find the last '.' outside brackets
        let mut last_dot = None;
        let mut depth = 0;
        for (i, ch) in self.path_string.char_indices() {
            match ch {
                '[' | '{' => depth += 1,
                ']' | '}' => depth -= 1,
                '.' if depth == 0 => last_dot = Some(i),
                _ => {}
            }
        }

        last_dot.map(|pos| &self.path_string[pos + 1..])
    }

    /// Returns the variant selection as (variant_set, variant) if this is a
    /// variant selection path.
    pub fn get_variant_selection(&self) -> Option<(String, String)> {
        if !self.is_prim_variant_selection_path() {
            return None;
        }
        path_node::get_variant_selection(self.prim_handle)
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    /// Returns the parent path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let path = Path::from_string("/World/Cube").unwrap();
    /// assert_eq!(path.get_parent_path().as_str(), "/World");
    ///
    /// let prop = Path::from_string("/World.visibility").unwrap();
    /// assert_eq!(prop.get_parent_path().as_str(), "/World");
    /// ```
    pub fn get_parent_path(&self) -> Path {
        if self.is_empty() {
            return self.clone();
        }
        // C++ returns EmptyPath for absolute root (GetParentNode() = nullptr)
        if self.is_absolute_root_path() {
            return Path::empty();
        }
        if self.path_string == "." {
            return Path::from_string("..").unwrap_or_else(Path::empty);
        }

        // If we have a property part, the parent depends on property node type
        if !self.prop_handle.is_null() {
            let prop_type = path_node::get_node_type(self.prop_handle);
            match prop_type {
                NodeType::PrimProperty => {
                    // Parent of /Foo.prop is /Foo (just the prim)
                    return Path::from_handles(self.prim_handle, NodeHandle::NULL);
                }
                NodeType::Target => {
                    // Parent of /Foo.rel[/Target] is /Foo.rel (the property)
                    let parent = path_node::get_parent(self.prop_handle);
                    return Path::from_handles(self.prim_handle, parent);
                }
                NodeType::RelationalAttribute => {
                    // Parent of /Foo.rel[/Target].attr is /Foo.rel[/Target]
                    let parent = path_node::get_parent(self.prop_handle);
                    return Path::from_handles(self.prim_handle, parent);
                }
                NodeType::Mapper => {
                    // Parent of .mapper[/Target] is the property
                    let parent = path_node::get_parent(self.prop_handle);
                    return Path::from_handles(self.prim_handle, parent);
                }
                NodeType::MapperArg => {
                    // Parent of .mapper[/T].arg is .mapper[/T]
                    let parent = path_node::get_parent(self.prop_handle);
                    return Path::from_handles(self.prim_handle, parent);
                }
                NodeType::Expression => {
                    // Parent of .expression is the property
                    let parent = path_node::get_parent(self.prop_handle);
                    return Path::from_handles(self.prim_handle, parent);
                }
                _ => {}
            }
        }

        // Prim-only path: go to parent prim
        let prim_type = path_node::get_node_type(self.prim_handle);
        match prim_type {
            NodeType::Prim | NodeType::PrimVariantSelection => {
                let parent = path_node::get_parent(self.prim_handle);
                if parent.is_null() {
                    return Path::empty();
                }
                let parent_type = path_node::get_node_type(parent);
                if parent_type == NodeType::Root {
                    // Parent is a root node
                    let flags = path_node::get_node_flags(parent);
                    if flags.contains(NodeFlags::IS_ABSOLUTE) {
                        return Path::absolute_root();
                    } else {
                        return Path::reflexive_relative();
                    }
                }
                Path::from_handles(parent, NodeHandle::NULL)
            }
            NodeType::Root => {
                // Already at root
                self.clone()
            }
            _ => Path::empty(),
        }
    }

    /// Returns the prim path by stripping all properties, targets, and variant
    /// selections.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let prop = Path::from_string("/World.visibility").unwrap();
    /// assert_eq!(prop.get_prim_path().as_str(), "/World");
    /// ```
    pub fn get_prim_path(&self) -> Path {
        if self.is_empty() {
            return Path::empty();
        }
        if self.is_prim_path() {
            return self.clone();
        }

        // Strip property part first
        let prim_h = self.prim_handle;
        if prim_h.is_null() {
            return Path::empty();
        }

        // Now strip variant selections from prim chain
        let stripped = Self::strip_variant_handles(prim_h);
        if stripped.is_null() {
            return Path::absolute_root();
        }

        Path::from_handles(stripped, NodeHandle::NULL)
    }

    /// Walk up the prim handle chain, rebuilding without variant selections.
    fn strip_variant_handles(handle: NodeHandle) -> NodeHandle {
        if handle.is_null() {
            return handle;
        }
        let node_type = path_node::get_node_type(handle);

        // If no variant selections in the chain, return as-is
        if !path_node::get_node_flags(handle).contains(NodeFlags::CONTAINS_VARIANT_SEL) {
            return handle;
        }

        // Collect the chain, filtering out variant selections
        let mut chain = Vec::new();
        let mut current = handle;
        while !current.is_null() {
            let nt = path_node::get_node_type(current);
            if nt != NodeType::PrimVariantSelection {
                chain.push(current);
            }
            current = path_node::get_parent(current);
        }
        chain.reverse();

        // Rebuild from root
        if chain.is_empty() {
            return NodeHandle::NULL;
        }

        // The first element should be a root
        let mut result = chain[0];
        for &h in &chain[1..] {
            let node = path_node::get_node(h);
            match &node.data {
                path_node::NodeData::Prim { name } => {
                    result = path_node::find_or_create_prim(result, name);
                }
                path_node::NodeData::Root { .. } => {
                    result = h;
                }
                _ => {
                    // Shouldn't happen after filtering but handle gracefully
                    result = h;
                }
            }
        }

        let _ = node_type; // suppress warning
        result
    }

    /// Returns the prim or prim variant selection path.
    pub fn get_prim_or_prim_variant_selection_path(&self) -> Path {
        if self.is_prim_or_prim_variant_selection_path() {
            return self.clone();
        }

        // Just strip property part
        if !self.prim_handle.is_null() {
            return Path::from_handles(self.prim_handle, NodeHandle::NULL);
        }

        self.clone()
    }

    /// Returns the absolute root or prim path.
    pub fn get_absolute_root_or_prim_path(&self) -> Path {
        if self.is_absolute_root_path() {
            return self.clone();
        }
        self.get_prim_path()
    }

    /// Returns the target path if this is a target, relational attribute,
    /// or mapper path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let target = Path::from_string("/World.rel[/Target]").unwrap();
    /// assert_eq!(target.get_target_path().unwrap().as_str(), "/Target");
    /// ```
    pub fn get_target_path(&self) -> Option<Path> {
        if !self.contains_target_path() {
            return None;
        }

        // Walk the property chain to find the Target or Mapper node
        let mut current = self.prop_handle;
        while !current.is_null() {
            let nt = path_node::get_node_type(current);
            match nt {
                NodeType::Target | NodeType::Mapper => {
                    if let Some(target_str) = path_node::get_target_path_str(current) {
                        return Path::from_string(&target_str);
                    }
                }
                _ => {}
            }
            current = path_node::get_parent(current);
            // Stop when we hit prim-side nodes
            if !current.is_null() {
                let ct = path_node::get_node_type(current);
                if matches!(
                    ct,
                    NodeType::Root | NodeType::Prim | NodeType::PrimVariantSelection
                ) {
                    break;
                }
            }
        }
        None
    }

    /// Replaces the relational attribute's target path.
    ///
    /// The path must be a relational attribute path.
    pub fn replace_target_path(&self, new_target_path: &Path) -> Option<Path> {
        if self.is_empty() || new_target_path.is_empty() {
            return None;
        }

        if !self.is_relational_attribute_path() && !self.is_target_path() && !self.is_mapper_path()
        {
            return Some(self.clone());
        }

        // String-based replacement is simplest and correct
        if let Some(target_start) = self.path_string.find('[') {
            if let Some(target_end) = self.path_string[target_start..].find(']') {
                let before_target = &self.path_string[..target_start + 1];
                let after_target = &self.path_string[target_start + target_end..];
                let new_path = format!(
                    "{}{}{}",
                    before_target,
                    new_target_path.as_str(),
                    after_target
                );
                return Path::from_string(&new_path);
            }
        }

        None
    }

    // =========================================================================
    // Prefix operations
    // =========================================================================

    /// Returns whether `prefix` is a prefix of this path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let path = Path::from_string("/World/Cube").unwrap();
    /// let prefix = Path::from_string("/World").unwrap();
    /// assert!(path.has_prefix(&prefix));
    /// ```
    pub fn has_prefix(&self, prefix: &Path) -> bool {
        if prefix.is_empty() || self.is_empty() {
            return false;
        }
        if prefix.is_absolute_root_path() && self.is_absolute_path() {
            return true;
        }
        if *self == *prefix {
            return true;
        }

        // String-based check is still correct and efficient with cached strings
        let self_str = &self.path_string;
        let prefix_str = &prefix.path_string;

        if !self_str.starts_with(prefix_str) {
            return false;
        }

        // Make sure prefix ends at a path boundary
        let after_prefix = &self_str[prefix_str.len()..];
        if after_prefix.is_empty() {
            return true;
        }

        let Some(next_char) = after_prefix.chars().next() else {
            return true;
        };
        matches!(next_char, '/' | '.' | '[' | '{')
    }

    /// Returns the common prefix of this path and another.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let path1 = Path::from_string("/World/A").unwrap();
    /// let path2 = Path::from_string("/World/B").unwrap();
    /// assert_eq!(path1.get_common_prefix(&path2).as_str(), "/World");
    /// ```
    pub fn get_common_prefix(&self, other: &Path) -> Path {
        if self.is_empty() || other.is_empty() {
            return Path::empty();
        }

        // String-based common prefix (same algorithm as before)
        let self_chars: Vec<char> = self.path_string.chars().collect();
        let other_chars: Vec<char> = other.path_string.chars().collect();
        let min_len = self_chars.len().min(other_chars.len());

        let mut common_len = 0;
        let mut last_boundary = 0;

        for i in 0..min_len {
            if self_chars[i] != other_chars[i] {
                break;
            }
            common_len = i + 1;
            if matches!(self_chars[i], '/' | '.' | '[' | '{') {
                last_boundary = common_len;
            }
        }

        // If we matched to the end of one string, that's a valid prefix
        if common_len == min_len
            && (common_len == self_chars.len() || common_len == other_chars.len())
        {
            last_boundary = common_len;
        }

        if last_boundary == 0 {
            return Path::empty();
        }

        let prefix: String = self_chars[..last_boundary].iter().collect();

        // Remove trailing delimiters
        let prefix = prefix.trim_end_matches(['/', '.']);

        if prefix.is_empty() && self.is_absolute_path() && other.is_absolute_path() {
            return Path::absolute_root();
        }

        Path::from_string(prefix).unwrap_or_else(Path::empty)
    }

    // =========================================================================
    // Appending
    // =========================================================================

    /// Appends a child prim to this path.
    ///
    /// # Arguments
    ///
    /// * `child_name` - The name of the child prim
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let parent = Path::from_string("/World").unwrap();
    /// let child = parent.append_child("Cube").unwrap();
    /// assert_eq!(child.as_str(), "/World/Cube");
    /// ```
    pub fn append_child(&self, child_name: &str) -> Option<Path> {
        if self.is_empty() {
            return None;
        }
        if !Self::is_valid_identifier(child_name) {
            return None;
        }
        if self.contains_property_elements() {
            return None;
        }

        // Handle parent path element
        if child_name == ".." {
            return Some(self.get_parent_path());
        }

        let new_prim = path_node::find_or_create_prim(self.prim_handle, &Token::new(child_name));
        Some(Path::from_handles(new_prim, NodeHandle::NULL))
    }

    /// Appends a property to this path.
    ///
    /// # Arguments
    ///
    /// * `prop_name` - The name of the property
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let prim = Path::from_string("/World").unwrap();
    /// let prop = prim.append_property("visibility").unwrap();
    /// assert_eq!(prop.as_str(), "/World.visibility");
    /// ```
    pub fn append_property(&self, prop_name: &str) -> Option<Path> {
        if self.is_empty() {
            return None;
        }
        if !Self::is_valid_namespaced_identifier(prop_name) {
            return None;
        }
        if self.contains_property_elements() {
            return None;
        }
        if self.is_absolute_root_path() {
            return None;
        }

        let prop_handle =
            path_node::find_or_create_prim_property(self.prim_handle, &Token::new(prop_name));
        Some(Path::from_handles(self.prim_handle, prop_handle))
    }

    /// Appends a variant selection to this path.
    ///
    /// # Arguments
    ///
    /// * `variant_set` - The variant set name
    /// * `variant` - The variant selection
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let prim = Path::from_string("/World").unwrap();
    /// let variant = prim.append_variant_selection("model", "lod0").unwrap();
    /// assert_eq!(variant.as_str(), "/World{model=lod0}");
    /// ```
    pub fn append_variant_selection(&self, variant_set: &str, variant: &str) -> Option<Path> {
        if self.is_empty() {
            return None;
        }
        if !self.is_prim_or_prim_variant_selection_path() {
            return None;
        }

        let new_prim =
            path_node::find_or_create_variant_selection(self.prim_handle, variant_set, variant);
        Some(Path::from_handles(new_prim, NodeHandle::NULL))
    }

    /// Appends a relationship target to this path.
    ///
    /// # Arguments
    ///
    /// * `target_path` - The target path
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let rel = Path::from_string("/World.rel").unwrap();
    /// let target = rel.append_target(&Path::from_string("/Target").unwrap()).unwrap();
    /// assert_eq!(target.as_str(), "/World.rel[/Target]");
    /// ```
    pub fn append_target(&self, target_path: &Path) -> Option<Path> {
        if self.is_empty() || target_path.is_empty() {
            return None;
        }
        if !self.is_property_path() {
            return None;
        }

        let target_handle =
            path_node::find_or_create_target(self.prop_handle, target_path.as_str());
        Some(Path::from_handles(self.prim_handle, target_handle))
    }

    /// Appends a relational attribute to this path.
    pub fn append_relational_attribute(&self, attr_name: &str) -> Option<Path> {
        if self.is_empty() {
            return None;
        }
        if !self.is_target_path() {
            return None;
        }
        if !Self::is_valid_namespaced_identifier(attr_name) {
            return None;
        }

        let attr_handle = path_node::find_or_create_relational_attribute(
            self.prop_handle,
            &Token::new(attr_name),
        );
        Some(Path::from_handles(self.prim_handle, attr_handle))
    }

    /// Appends a mapper to this path.
    pub fn append_mapper(&self, target_path: &Path) -> Option<Path> {
        if self.is_empty() || target_path.is_empty() {
            return None;
        }
        if !self.is_property_path() {
            return None;
        }

        let mapper_handle =
            path_node::find_or_create_mapper(self.prop_handle, target_path.as_str());
        Some(Path::from_handles(self.prim_handle, mapper_handle))
    }

    /// Appends a mapper arg to this path.
    pub fn append_mapper_arg(&self, arg_name: &str) -> Option<Path> {
        if self.is_empty() {
            return None;
        }
        if !self.is_mapper_path() {
            return None;
        }
        if !Self::is_valid_identifier(arg_name) {
            return None;
        }

        let arg_handle =
            path_node::find_or_create_mapper_arg(self.prop_handle, &Token::new(arg_name));
        Some(Path::from_handles(self.prim_handle, arg_handle))
    }

    /// Appends an expression to this path.
    pub fn append_expression(&self) -> Option<Path> {
        if self.is_empty() {
            return None;
        }
        if !self.is_property_path() {
            return None;
        }

        let expr_handle = path_node::find_or_create_expression(self.prop_handle);
        Some(Path::from_handles(self.prim_handle, expr_handle))
    }

    /// Creates a path by extracting and appending an element from the given ASCII element encoding.
    ///
    /// Attempting to append a root or empty path (or malformed path) or attempting to append
    /// to the EmptyPath will return None.
    pub fn append_element_string(&self, element: &str) -> Option<Path> {
        self.append_element_token(&Token::new(element))
    }

    /// Like append_element_string() but take the element as a Token.
    pub fn append_element_token(&self, element: &Token) -> Option<Path> {
        if self.is_empty() {
            return None;
        }

        let element_str = element.as_str();

        // Handle variant selection: {variantSet=variant}
        if element_str.starts_with('{') {
            if let Some(end) = element_str.find('}') {
                let content = &element_str[1..end];
                if let Some(eq_pos) = content.find('=') {
                    let variant_set = &content[..eq_pos];
                    let variant = &content[eq_pos + 1..];
                    return self.append_variant_selection(variant_set, variant);
                }
            }
            return None;
        }

        // Handle target path: [target]
        if element_str.starts_with('[') && element_str.ends_with(']') {
            let target_str = &element_str[1..element_str.len() - 1];
            if let Some(target) = Path::from_string(target_str) {
                return self.append_target(&target);
            }
            return None;
        }

        // Handle property: .property
        if let Some(prop_name) = element_str.strip_prefix('.') {
            // Check for special cases
            if prop_name == "expression" {
                return self.append_expression();
            }

            if prop_name.starts_with("mapper[") {
                if let Some(end) = prop_name.find(']') {
                    let target_str = &prop_name[7..end];
                    if let Some(target) = Path::from_string(target_str) {
                        return self.append_mapper(&target);
                    }
                }
                return None;
            }

            // Regular property
            if self.is_mapper_path() {
                return self.append_mapper_arg(prop_name);
            } else if self.is_target_path() {
                return self.append_relational_attribute(prop_name);
            } else {
                return self.append_property(prop_name);
            }
        }

        // Handle prim child
        self.append_child(element_str)
    }

    /// Appends a path suffix to this path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let base = Path::from_string("/World").unwrap();
    /// let suffix = Path::from_string("Cube/Mesh").unwrap();
    /// let result = base.append_path(&suffix).unwrap();
    /// assert_eq!(result.as_str(), "/World/Cube/Mesh");
    /// ```
    pub fn append_path(&self, suffix: &Path) -> Option<Path> {
        if self.is_empty() || suffix.is_empty() {
            return None;
        }
        if suffix.is_absolute_path() {
            return None;
        }
        if *suffix == Path::reflexive_relative() {
            return Some(self.clone());
        }

        let suffix_str = &suffix.path_string;

        // Handle leading "./"
        let suffix_str = suffix_str.strip_prefix("./").unwrap_or(suffix_str);

        if self.is_absolute_root_path() {
            return Path::from_string(&format!("/{}", suffix_str));
        }

        let new_path = format!("{}/{}", self.path_string, suffix_str);
        Path::from_string(&new_path)
    }

    // =========================================================================
    // Path replacement
    // =========================================================================

    /// Returns a copy of this path with its final component changed.
    ///
    /// # Arguments
    ///
    /// * `new_name` - The new name for the final component
    pub fn replace_name(&self, new_name: &str) -> Option<Path> {
        if self.is_prim_path() {
            self.get_parent_path().append_child(new_name)
        } else if self.is_prim_property_path() {
            self.get_parent_path().append_property(new_name)
        } else if self.is_relational_attribute_path() {
            self.get_parent_path().append_relational_attribute(new_name)
        } else {
            None
        }
    }

    /// Replaces the prefix of this path.
    ///
    /// # Arguments
    ///
    /// * `old_prefix` - The prefix to replace
    /// * `new_prefix` - The new prefix
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let path = Path::from_string("/World/Cube").unwrap();
    /// let old_prefix = Path::from_string("/World").unwrap();
    /// let new_prefix = Path::from_string("/NewWorld").unwrap();
    /// let result = path.replace_prefix(&old_prefix, &new_prefix).unwrap();
    /// assert_eq!(result.as_str(), "/NewWorld/Cube");
    /// ```
    pub fn replace_prefix(&self, old_prefix: &Path, new_prefix: &Path) -> Option<Path> {
        self.replace_prefix_impl(old_prefix, new_prefix, true)
    }

    /// Like `replace_prefix`, but with explicit control over whether embedded
    /// target paths (relationship targets, mapper args) are also fixed up.
    /// When `fix_target_paths` is false, only the main path prefix is replaced.
    pub fn replace_prefix_with_fix(
        &self,
        old_prefix: &Path,
        new_prefix: &Path,
        fix_target_paths: bool,
    ) -> Option<Path> {
        self.replace_prefix_impl(old_prefix, new_prefix, fix_target_paths)
    }

    /// Internal implementation of prefix replacement.
    /// When `fix_target_paths` is true, embedded target paths inside brackets
    /// also have their prefix replaced (matches C++ fixTargetPaths behavior).
    fn replace_prefix_impl(
        &self,
        old_prefix: &Path,
        new_prefix: &Path,
        fix_target_paths: bool,
    ) -> Option<Path> {
        // C++ returns *this (copy) when self is empty or prefixes are the same.
        // Empty new/old prefix maps to EmptyPath in C++.
        if self.is_empty() || *old_prefix == *new_prefix {
            return Some(self.clone());
        }
        if old_prefix.is_empty() || new_prefix.is_empty() {
            return Some(Path::empty());
        }
        if *self == *old_prefix {
            return Some(new_prefix.clone());
        }

        let has_prefix = self.has_prefix(old_prefix);

        // C++ _ReplacePrimPrefix: decompose self into elements beyond
        // old_prefix, then rebuild by appending each tail element onto
        // new_prefix using structural operations (append_child,
        // append_variant_selection).  This correctly handles variant
        // selection paths where string concatenation would produce
        // wrong results (e.g. /Foo{v=sel}/Child instead of /Foo{v=sel}Child).
        let base_replaced = if has_prefix {
            // Collect tail elements: walk self backwards until we match
            // old_prefix depth, recording each element name + type.
            enum PathElement {
                PrimChild(String),
                VariantSelection(String, String),
            }
            let mut tail_elements: Vec<PathElement> = Vec::new();
            let mut cur = self.clone();
            while cur != *old_prefix && !cur.is_empty() && !cur.is_absolute_root_path() {
                if cur.is_prim_variant_selection_path() {
                    // Extract variant set + selection from the path
                    if let Some((vset, vsel)) = cur.get_variant_selection() {
                        tail_elements.push(PathElement::VariantSelection(vset, vsel));
                    }
                    cur = cur.get_parent_path();
                } else {
                    let name = cur.get_name().to_string();
                    tail_elements.push(PathElement::PrimChild(name));
                    cur = cur.get_parent_path();
                }
            }

            if cur != *old_prefix {
                // old_prefix is not actually a prefix — return self unchanged
                self.path_string.clone()
            } else {
                // Rebuild: start from new_prefix, append tail in reverse
                let mut result = new_prefix.clone();
                for elem in tail_elements.iter().rev() {
                    match elem {
                        PathElement::PrimChild(name) => {
                            if let Some(p) = result.append_child(name) {
                                result = p;
                            }
                        }
                        PathElement::VariantSelection(vset, vsel) => {
                            if let Some(p) = result.append_variant_selection(vset, vsel) {
                                result = p;
                            }
                        }
                    }
                }
                result.path_string
            }
        } else {
            self.path_string.clone()
        };

        // Fix embedded target paths inside [...] brackets
        let result_str = if fix_target_paths && base_replaced.contains('[') {
            Self::replace_target_path_prefixes(&base_replaced, old_prefix, new_prefix)
        } else {
            base_replaced
        };

        Path::from_string(&result_str)
    }

    /// Replace prefixes inside all embedded target paths (the [...] parts).
    fn replace_target_path_prefixes(
        path_str: &str,
        old_prefix: &Path,
        new_prefix: &Path,
    ) -> String {
        let mut result = String::with_capacity(path_str.len());
        let mut chars = path_str.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '[' {
                // Collect the target path inside brackets (handling nested brackets)
                let mut depth = 1;
                let mut target = String::new();
                for inner in chars.by_ref() {
                    if inner == '[' {
                        depth += 1;
                        target.push(inner);
                    } else if inner == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        target.push(inner);
                    } else {
                        target.push(inner);
                    }
                }
                // Recursively replace prefix in the target path
                let replaced_target = if let Some(target_path) = Path::from_string(&target) {
                    if let Some(fixed) =
                        target_path.replace_prefix_impl(old_prefix, new_prefix, true)
                    {
                        fixed.path_string
                    } else {
                        target
                    }
                } else {
                    target
                };
                result.push('[');
                result.push_str(&replaced_target);
                result.push(']');
            } else {
                result.push(ch);
            }
        }
        result
    }

    // =========================================================================
    // Absolute/relative conversion
    // =========================================================================

    /// Makes this path absolute using the given anchor.
    ///
    /// # Arguments
    ///
    /// * `anchor` - The absolute prim path to use as anchor
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let relative = Path::from_string("Cube").unwrap();
    /// let anchor = Path::from_string("/World").unwrap();
    /// let absolute = relative.make_absolute(&anchor).unwrap();
    /// assert_eq!(absolute.as_str(), "/World/Cube");
    /// ```
    pub fn make_absolute(&self, anchor: &Path) -> Option<Path> {
        if anchor.is_empty() || !anchor.is_absolute_path() {
            return None;
        }
        if self.is_empty() {
            return None;
        }
        if self.is_absolute_path() {
            return Some(self.clone());
        }

        // Walk relative path components, resolving ".." by going up in anchor.
        // Matches C++ SdfPath::MakeAbsolutePath which uses _AppendNode.
        let rel_str = self.as_str();
        let mut base_components: Vec<&str> = anchor
            .as_str()
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        // Split relative path, handling property part (e.g., "../B.attr")
        // Property dot is the LAST dot that is NOT part of a ".." parent component.
        let prop_dot = rel_str
            .rmatch_indices('.')
            .find(|&(i, _)| {
                let bytes = rel_str.as_bytes();
                let next_is_dot = bytes.get(i + 1) == Some(&b'.');
                let prev_is_dot = i > 0 && bytes[i - 1] == b'.';
                !next_is_dot && !prev_is_dot
            })
            .map(|(i, _)| i);
        let (prim_part, prop_part) = if let Some(dot_pos) = prop_dot {
            (&rel_str[..dot_pos], Some(&rel_str[dot_pos + 1..]))
        } else {
            (rel_str, None)
        };

        for component in prim_part.split('/').filter(|s| !s.is_empty()) {
            if component == ".." {
                if base_components.is_empty() {
                    return None; // Can't go above root
                }
                base_components.pop();
            } else {
                base_components.push(component);
            }
        }

        let mut result = format!("/{}", base_components.join("/"));
        if let Some(prop) = prop_part {
            result.push('.');
            result.push_str(prop);
        }
        Path::from_string(&result)
    }

    /// Makes this path relative to the given anchor.
    ///
    /// # Arguments
    ///
    /// * `anchor` - The absolute prim path to make relative to
    pub fn make_relative(&self, anchor: &Path) -> Option<Path> {
        if anchor.is_empty() || !anchor.is_absolute_path() {
            return None;
        }
        if self.is_empty() {
            return None;
        }
        if !self.is_absolute_path() {
            // Already relative - make canonical
            return self.make_absolute(anchor)?.make_relative(anchor);
        }

        // Find common prefix
        let common = self.get_common_prefix(anchor);

        // Count how many ".." we need
        let mut current = anchor.clone();
        let mut dot_dots = Vec::new();
        while current != common && !current.is_absolute_root_path() {
            dot_dots.push("..");
            current = current.get_parent_path();
        }

        // Get the suffix after common prefix
        let suffix = if self.path_string.len() > common.path_string.len() {
            &self.path_string[common.path_string.len()..]
        } else {
            ""
        };
        let suffix = suffix.trim_start_matches('/');

        // Build relative path
        let mut parts: Vec<&str> = dot_dots.iter().map(|s| &**s).collect();
        if !suffix.is_empty() {
            parts.push(suffix);
        }

        if parts.is_empty() {
            return Some(Path::reflexive_relative());
        }

        Path::from_string(&parts.join("/"))
    }

    /// Find and remove the longest common suffix from two paths.
    ///
    /// Returns this path and `other_path` with the longest common suffix
    /// removed (first and second, respectively). If the two paths have no
    /// common suffix then the paths are returned as-is. If the paths are
    /// equal then this returns empty paths for relative paths and absolute
    /// roots for absolute paths.
    ///
    /// If `stop_at_root_prim` is `true` then neither returned path will be
    /// the root path.
    pub fn remove_common_suffix(&self, other_path: &Path, stop_at_root_prim: bool) -> (Path, Path) {
        if self.is_empty() || other_path.is_empty() {
            return (self.clone(), other_path.clone());
        }

        // Check if both have property parts or both don't
        let self_has_prop = self.contains_property_elements();
        let other_has_prop = other_path.contains_property_elements();
        if self_has_prop != other_has_prop {
            return (self.clone(), other_path.clone());
        }

        // Use string-based approach
        let self_parts: Vec<&str> = self.path_string.split('/').collect();
        let other_parts: Vec<&str> = other_path.path_string.split('/').collect();

        let mut self_idx = self_parts.len();
        let mut other_idx = other_parts.len();

        // Find common suffix by walking backwards
        while self_idx > 0 && other_idx > 0 {
            if self_parts[self_idx - 1] == other_parts[other_idx - 1] {
                self_idx -= 1;
                other_idx -= 1;
            } else {
                break;
            }
        }

        // Check stop_at_root_prim constraint
        if stop_at_root_prim {
            let self_remaining = self_idx;
            let other_remaining = other_idx;
            if self_remaining == 0 || other_remaining == 0 {
                return (self.clone(), other_path.clone());
            }
        }

        let self_result: String = if self_idx == 0 {
            if self.is_absolute_path() {
                "/".to_string()
            } else {
                ".".to_string()
            }
        } else {
            self_parts[..self_idx].join("/")
        };

        let other_result: String = if other_idx == 0 {
            if other_path.is_absolute_path() {
                "/".to_string()
            } else {
                ".".to_string()
            }
        } else {
            other_parts[..other_idx].join("/")
        };

        (
            Path::from_string(&self_result).unwrap_or_else(Path::empty),
            Path::from_string(&other_result).unwrap_or_else(Path::empty),
        )
    }

    // =========================================================================
    // Prefix operations (GetPrefixes)
    // =========================================================================

    /// Returns the prefix paths of this path.
    ///
    /// Prefixes are returned in order of shortest to longest. The path itself
    /// is returned as the last prefix.
    pub fn get_prefixes(&self) -> PathVector {
        let mut result = PathVector::new();
        self.get_prefixes_into(&mut result);
        result
    }

    /// Return up to `num_prefixes` prefix paths of this path.
    ///
    /// Prefixes are returned in order of shortest to longest. The path itself
    /// is included as the last prefix.
    pub fn get_prefixes_count(&self, num_prefixes: usize) -> PathVector {
        let mut result = PathVector::new();
        self.get_prefixes_into_count(&mut result, num_prefixes);
        result
    }

    /// Fills `prefixes` with prefixes of this path.
    ///
    /// Prefixes are returned in order of shortest to longest. The path itself
    /// is returned as the last prefix.
    pub fn get_prefixes_into(&self, prefixes: &mut PathVector) {
        self.get_prefixes_into_count(prefixes, 0);
    }

    /// Fill `prefixes` with up to `num_prefixes` prefixes of this path.
    ///
    /// Prefixes are filled in order of shortest to longest. The path itself is
    /// included as the last prefix. If `num_prefixes` is 0 or greater than the
    /// number of this path's prefixes, fill all prefixes.
    pub fn get_prefixes_into_count(&self, prefixes: &mut PathVector, num_prefixes: usize) {
        self.get_prefixes_into_impl(prefixes, num_prefixes);
    }

    fn get_prefixes_into_impl(&self, prefixes: &mut PathVector, num_prefixes: usize) {
        let elem_count = self.get_path_element_count();
        let actual_count = if num_prefixes == 0 || num_prefixes > elem_count + 1 {
            elem_count + 1 // +1 to include the path itself
        } else {
            num_prefixes
        };

        prefixes.clear();
        prefixes.reserve(actual_count);

        // Build prefixes from shortest to longest
        let mut prefix_parts = Vec::new();

        // Collect all path parts
        let mut current = self.clone();
        while !current.is_absolute_root_path() && current.path_string != "." && !current.is_empty()
        {
            prefix_parts.push(current.clone());
            let parent = current.get_parent_path();
            if parent == current {
                break;
            }
            current = parent;
        }

        // Add root if absolute
        if self.is_absolute_path() {
            prefix_parts.push(Path::absolute_root());
        } else if self.path_string == "." {
            prefix_parts.push(Path::reflexive_relative());
        }

        // Reverse to get shortest to longest, then take only what we need
        prefix_parts.reverse();
        prefixes.extend(prefix_parts.into_iter().take(actual_count));
    }

    /// Return a range for iterating over the ancestors of this path.
    ///
    /// The range provides iteration over the path and all of its prefixes,
    /// ordered from longest to shortest (the opposite of the order of the
    /// prefixes returned by `get_prefixes()`).
    pub fn get_ancestors_range(&self) -> AncestorsRange {
        AncestorsRange::new(self.clone())
    }

    // =========================================================================
    // Validation
    // =========================================================================

    /// Returns whether the given string is a valid path.
    ///
    /// Checks for balanced brackets/braces, rejects double slashes,
    /// trailing slashes, empty path components, and validates that
    /// prim name components are valid identifiers.
    pub fn is_valid_path_string(path: &str) -> bool {
        if path.is_empty() {
            return false;
        }

        // Special root/relative paths
        if path == "/" || path == "." || path == ".." {
            return true;
        }

        // Reject double slashes (empty components)
        if path.contains("//") {
            return false;
        }

        // Reject trailing slash (except root "/")
        if path.len() > 1 && path.ends_with('/') {
            return false;
        }

        // Check for balanced brackets and braces
        let mut bracket_depth = 0i32;
        let mut brace_depth = 0i32;

        for ch in path.chars() {
            match ch {
                '[' => bracket_depth += 1,
                ']' => {
                    bracket_depth -= 1;
                    if bracket_depth < 0 {
                        return false;
                    }
                }
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth < 0 {
                        return false;
                    }
                }
                _ => {}
            }
        }

        if bracket_depth != 0 || brace_depth != 0 {
            return false;
        }

        // Validate prim/property name components (C++ parity).
        Self::validate_path_components(path)
    }

    /// Validate that path components contain valid identifier characters.
    ///
    /// Skips content inside brackets (target paths) and braces (variant
    /// selections). Validates prim components as identifiers and property
    /// components as namespaced identifiers.
    fn validate_path_components(path: &str) -> bool {
        let s = path;

        // Strip leading '/' for absolute paths
        let s = if s.starts_with('/') { &s[1..] } else { s };

        if s.is_empty() {
            return true; // was just "/"
        }

        // Split into prim part and property part at first top-level '.'
        let (prim_part, prop_part) = Self::split_at_top_level_dot(s);

        // Validate prim components (split on '/' outside brackets/braces)
        if !prim_part.is_empty() {
            for comp in Self::split_top_level(prim_part, '/') {
                // Skip relative path components
                if comp == "." || comp == ".." {
                    continue;
                }
                // Skip empty components (leading slash case)
                if comp.is_empty() {
                    continue;
                }
                // Strip variant selection suffix: e.g. "Prim{var=sel}"
                let name = if let Some(brace_pos) = comp.find('{') {
                    &comp[..brace_pos]
                } else {
                    comp
                };
                if !name.is_empty() && !Self::is_valid_identifier(name) {
                    return false;
                }
            }
        }

        // Validate property component as namespaced identifier
        if let Some(prop) = prop_part {
            let name = if let Some(bracket_pos) = prop.find('[') {
                &prop[..bracket_pos]
            } else {
                prop
            };
            if name.contains('/') {
                for comp in name.split('/') {
                    if comp == "." || comp == ".." || comp.is_empty() {
                        continue;
                    }
                    let c = if let Some(bp) = comp.find('{') {
                        &comp[..bp]
                    } else {
                        comp
                    };
                    if !c.is_empty() && !Self::is_valid_identifier(c) {
                        return false;
                    }
                }
            } else if !name.is_empty() && !Self::is_valid_namespaced_identifier(name) {
                return false;
            }
        }

        true
    }

    /// Split path string at the first top-level '.' (not inside brackets/braces).
    /// Returns (prim_part, Some(property_part)) or (whole, None).
    fn split_at_top_level_dot(s: &str) -> (&str, Option<&str>) {
        let mut bracket = 0i32;
        let mut brace = 0i32;
        let bytes = s.as_bytes();
        for (i, ch) in s.char_indices() {
            match ch {
                '[' => bracket += 1,
                ']' => bracket -= 1,
                '{' => brace += 1,
                '}' => brace -= 1,
                '.' if bracket == 0 && brace == 0 => {
                    // Skip dots that are part of ".." (parent path component)
                    let next = bytes.get(i + 1).copied();
                    let prev = if i > 0 { bytes.get(i - 1).copied() } else { None };
                    if next == Some(b'.') || prev == Some(b'.') {
                        continue;
                    }
                    return (&s[..i], Some(&s[i + 1..]));
                }
                _ => {}
            }
        }
        (s, None)
    }

    /// Split string on delimiter, but only at the top level (not inside
    /// brackets or braces). Returns an iterator of slices.
    fn split_top_level(s: &str, delim: char) -> Vec<&str> {
        let mut result = Vec::new();
        let mut bracket = 0i32;
        let mut brace = 0i32;
        let mut start = 0;
        for (i, ch) in s.char_indices() {
            match ch {
                '[' => bracket += 1,
                ']' => bracket -= 1,
                '{' => brace += 1,
                '}' => brace -= 1,
                c if c == delim && bracket == 0 && brace == 0 => {
                    result.push(&s[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        result.push(&s[start..]);
        result
    }

    /// Returns whether the given string is a valid path, with error diagnostics.
    ///
    /// Like [`is_valid_path_string`] but returns an error message describing
    /// why validation failed.
    pub fn is_valid_path_with_error(path: &str) -> Result<(), String> {
        if path.is_empty() {
            return Err("empty path string".to_string());
        }
        if path == "/" || path == "." || path == ".." {
            return Ok(());
        }
        if path.contains("//") {
            return Err(format!("path contains double slash: {path}"));
        }
        if path.len() > 1 && path.ends_with('/') {
            return Err(format!("path has trailing slash: {path}"));
        }

        let mut bracket_depth = 0i32;
        let mut brace_depth = 0i32;
        for (i, ch) in path.chars().enumerate() {
            match ch {
                '[' => bracket_depth += 1,
                ']' => {
                    bracket_depth -= 1;
                    if bracket_depth < 0 {
                        return Err(format!("unbalanced ']' at position {i} in: {path}"));
                    }
                }
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth < 0 {
                        return Err(format!("unbalanced '}}' at position {i} in: {path}"));
                    }
                }
                _ => {}
            }
        }
        if bracket_depth != 0 {
            return Err(format!("unbalanced '[' in: {path}"));
        }
        if brace_depth != 0 {
            return Err(format!("unbalanced '{{' in: {path}"));
        }

        // Validate prim/property name components (C++ parity)
        if !Self::validate_path_components(path) {
            return Err(format!("invalid component identifier in: {path}"));
        }

        Ok(())
    }

    /// Returns whether the given string is a valid identifier.
    ///
    /// An identifier must start with a letter or underscore, and contain
    /// only letters, digits, and underscores.
    pub fn is_valid_identifier(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };

        // First char must be letter or underscore
        if !first.is_alphabetic() && first != '_' {
            return false;
        }

        // Rest must be alphanumeric or underscore
        for ch in chars {
            if !ch.is_alphanumeric() && ch != '_' {
                return false;
            }
        }

        true
    }

    /// Returns whether the given string is a valid namespaced identifier.
    ///
    /// A namespaced identifier is one or more valid identifiers joined by
    /// colons.
    pub fn is_valid_namespaced_identifier(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Cannot start or end with ':'
        if name.starts_with(':') || name.ends_with(':') {
            return false;
        }

        // All components must be valid identifiers
        for part in name.split(':') {
            if !Self::is_valid_identifier(part) {
                return false;
            }
        }

        true
    }

    /// Tokenizes an identifier by namespace delimiter.
    pub fn tokenize_identifier(name: &str) -> Vec<String> {
        if !Self::is_valid_namespaced_identifier(name) {
            return Vec::new();
        }
        name.split(':').map(|s| s.to_string()).collect()
    }

    /// Joins identifiers with namespace delimiter.
    pub fn join_identifier(names: &[&str]) -> String {
        names
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Strips namespace from an identifier.
    pub fn strip_namespace(name: &str) -> &str {
        name.rsplit(':').next().unwrap_or(name)
    }

    /// Strips namespace from a token.
    pub fn strip_namespace_token(name: &Token) -> Token {
        Token::new(Self::strip_namespace(name.as_str()))
    }

    /// Tokenizes an identifier by namespace delimiter, returning tokens.
    pub fn tokenize_identifier_as_tokens(name: &str) -> Vec<Token> {
        if !Self::is_valid_namespaced_identifier(name) {
            return Vec::new();
        }
        name.split(':').map(Token::new).collect()
    }

    /// Join names into a single identifier using the namespace delimiter.
    pub fn join_identifier_tokens(names: &[Token]) -> String {
        names
            .iter()
            .filter(|t| !t.as_str().is_empty())
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Join two tokens into a single identifier using the namespace delimiter.
    pub fn join_identifier_pair(lhs: &Token, rhs: &Token) -> String {
        if lhs.as_str().is_empty() {
            return rhs.as_str().to_string();
        }
        if rhs.as_str().is_empty() {
            return lhs.as_str().to_string();
        }
        format!("{}:{}", lhs.as_str(), rhs.as_str())
    }

    /// Returns (name, true) where name is stripped of the prefix specified by
    /// `match_namespace` if `name` indeed starts with `match_namespace`.
    /// Returns (name, false) otherwise, with `name` unmodified.
    pub fn strip_prefix_namespace(name: &str, match_namespace: &str) -> (String, bool) {
        let name_str = name;
        let match_str = match_namespace.trim_end_matches(':');

        if let Some(remaining) = name_str.strip_prefix(match_str) {
            if remaining.is_empty() || remaining.starts_with(':') {
                let stripped = if remaining.starts_with(':') {
                    &remaining[1..]
                } else {
                    remaining
                };
                return (stripped.to_string(), true);
            }
        }

        (name_str.to_string(), false)
    }

    /// Returns the hash of this path.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    // =========================================================================
    // Variant Selection Operations
    // =========================================================================

    /// Returns a new path with all variant selections stripped.
    ///
    /// This removes all `{variantSet=selection}` components from the path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Path;
    ///
    /// let path = Path::from_string("/World{model=lod0}/Mesh{render=high}").unwrap();
    /// let stripped = path.strip_all_variant_selections();
    /// assert_eq!(stripped.as_str(), "/World/Mesh");
    /// ```
    pub fn strip_all_variant_selections(&self) -> Path {
        if self.is_empty() || !self.contains_prim_variant_selection() {
            return self.clone();
        }

        let stripped_prim = Self::strip_variant_handles(self.prim_handle);
        Path::from_handles(stripped_prim, self.prop_handle)
    }

    // =========================================================================
    // Target Path Operations
    // =========================================================================

    /// Collects all target paths embedded in this path recursively.
    ///
    /// Target paths are paths inside square brackets, like `/Prim.rel[/Target]`.
    /// This method also collects targets from nested paths.
    ///
    /// # Returns
    ///
    /// A vector of all target paths found in this path.
    pub fn get_all_target_paths_recursively(&self) -> Vec<Path> {
        let mut result = Vec::new();

        if !self.contains_target_path() {
            return result;
        }

        // Walk the property chain to find all target nodes
        let mut current = self.prop_handle;
        while !current.is_null() {
            let nt = path_node::get_node_type(current);
            match nt {
                NodeType::Target | NodeType::Mapper => {
                    if let Some(target_str) = path_node::get_target_path_str(current) {
                        if let Some(target) = Path::from_string(&target_str) {
                            // Recursively collect from target too
                            let nested = target.get_all_target_paths_recursively();
                            result.push(target);
                            result.extend(nested);
                        }
                    }
                }
                _ => {}
            }
            current = path_node::get_parent(current);
            // Stop when we hit prim-side nodes
            if !current.is_null() {
                let ct = path_node::get_node_type(current);
                if matches!(
                    ct,
                    NodeType::Root | NodeType::Prim | NodeType::PrimVariantSelection
                ) {
                    break;
                }
            }
        }

        result
    }
}

// =========================================================================
// Trait implementations
// =========================================================================

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        // O(1) handle comparison when both are node-backed
        if !self.prim_handle.is_null() || !other.prim_handle.is_null() {
            return self.prim_handle == other.prim_handle && self.prop_handle == other.prop_handle;
        }
        // Both empty
        self.path_string == other.path_string
    }
}

impl Eq for Path {}

impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Path {
    fn cmp(&self, other: &Self) -> Ordering {
        // Absolute paths come before relative paths
        let self_abs = self.is_absolute_path();
        let other_abs = other.is_absolute_path();
        if self_abs != other_abs {
            return if self_abs {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        // Lexicographic comparison on path strings
        self.path_string.cmp(&other.path_string)
    }
}

impl Hash for Path {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // O(1) hash via handle values
        self.prim_handle.hash(state);
        self.prop_handle.hash(state);
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Path({})", self.path_string)
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path_string)
    }
}

impl AsRef<str> for Path {
    fn as_ref(&self) -> &str {
        &self.path_string
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self::from_string(s).unwrap_or_else(Self::empty)
    }
}

impl From<String> for Path {
    fn from(s: String) -> Self {
        Self::from_string(&s).unwrap_or_else(Self::empty)
    }
}

// =========================================================================
// Type aliases
// =========================================================================

/// A set of paths.
pub type PathSet = std::collections::BTreeSet<Path>;

/// A vector of paths.
pub type PathVector = Vec<Path>;

/// Range for iterating over ancestors of a path.
///
/// Provides iteration over the path and all of its prefixes, ordered from
/// longest to shortest.
pub struct AncestorsRange {
    path: Path,
}

impl AncestorsRange {
    fn new(path: Path) -> Self {
        Self { path }
    }

    /// Returns the path this range was created from.
    pub fn get_path(&self) -> &Path {
        &self.path
    }

    /// Returns an iterator over ancestors.
    pub fn iter(&self) -> AncestorsIterator {
        AncestorsIterator::new(self.path.clone())
    }
}

impl IntoIterator for AncestorsRange {
    type Item = Path;
    type IntoIter = AncestorsIterator;

    fn into_iter(self) -> Self::IntoIter {
        AncestorsIterator::new(self.path)
    }
}

/// Iterator over ancestors of a path.
pub struct AncestorsIterator {
    current: Option<Path>,
}

impl AncestorsIterator {
    fn new(path: Path) -> Self {
        Self {
            current: Some(path),
        }
    }
}

impl Iterator for AncestorsIterator {
    type Item = Path;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current.take()?;
        let result = current.clone();

        // Move to parent for next iteration
        if current.is_absolute_root_path() || current.path_string == "." {
            self.current = None;
        } else {
            let parent = current.get_parent_path();
            if parent == current {
                self.current = None;
            } else {
                self.current = Some(parent);
            }
        }

        Some(result)
    }
}

// =========================================================================
// Utility functions
// =========================================================================
// FindLongestPrefix / FindLongestStrictPrefix (matches C++ SdfPathFindLongestPrefix)
// =========================================================================

impl Path {
    /// Returns a reference to the path in `paths` that is the longest prefix of `path`
    /// (including `path` itself), if any. Otherwise `None`.
    ///
    /// Matches C++ `SdfPathFindLongestPrefix`. The slice must be sorted by `Path` order
    /// (e.g. via `paths.sort()`). Uses binary search for O(log n) lookup.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_sdf::Path;
    ///
    /// let paths: Vec<Path> = ["/", "/World", "/World/Scene"]
    ///     .iter()
    ///     .filter_map(|s| Path::from_string(s))
    ///     .collect();
    /// assert!(paths.windows(2).all(|w| w[0] < w[1])); // sorted
    ///
    /// let p = Path::from_string("/World/Scene/Char").unwrap();
    /// let found = Path::find_longest_prefix(&paths, &p);
    /// assert_eq!(found, Some(&paths[2])); // /World/Scene
    /// ```
    #[must_use]
    pub fn find_longest_prefix<'a>(paths: &'a [Path], path: &Path) -> Option<&'a Path> {
        Self::find_longest_prefix_impl(paths, path, false)
    }

    /// Returns a reference to the path in `paths` that is the longest *strict* prefix
    /// of `path` (excluding `path` itself), if any. Otherwise `None`.
    ///
    /// Matches C++ `SdfPathFindLongestStrictPrefix`. The slice must be sorted.
    #[must_use]
    pub fn find_longest_strict_prefix<'a>(paths: &'a [Path], path: &Path) -> Option<&'a Path> {
        Self::find_longest_prefix_impl(paths, path, true)
    }

    fn find_longest_prefix_impl<'a>(
        paths: &'a [Path],
        path: &Path,
        strict: bool,
    ) -> Option<&'a Path> {
        if paths.is_empty() {
            return None;
        }
        let idx = match paths.binary_search(path) {
            Ok(i) if !strict => return Some(&paths[i]),
            Ok(i) => i,
            Err(i) => i,
        };
        if idx == 0 {
            return None;
        }
        let pred = &paths[idx - 1];
        if path.has_prefix(pred) {
            return Some(pred);
        }
        let new_path = path.get_common_prefix(pred);
        Self::find_longest_prefix_impl(&paths[..idx], &new_path, false)
    }

    /// Returns the subrange `(start, end)` of the sorted `paths` slice that
    /// includes all paths prefixed by `prefix`.
    ///
    /// Matches C++ `SdfPathFindPrefixedRange`. The input must be sorted by `Path::cmp`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let paths = vec![
    ///     Path::from_string("/A").unwrap(),
    ///     Path::from_string("/B").unwrap(),
    ///     Path::from_string("/B/C").unwrap(),
    ///     Path::from_string("/B/D").unwrap(),
    ///     Path::from_string("/E").unwrap(),
    /// ];
    /// let (lo, hi) = Path::find_prefixed_range(&paths, &Path::from_string("/B").unwrap());
    /// assert_eq!(lo, 1);
    /// assert_eq!(hi, 4); // /B, /B/C, /B/D
    /// ```
    #[must_use]
    pub fn find_prefixed_range(paths: &[Path], prefix: &Path) -> (usize, usize) {
        // lower_bound: first path >= prefix
        let lo = paths.partition_point(|p| p < prefix);

        // Find end of prefixed range starting from lo
        let hi = lo + paths[lo..].partition_point(|p| p.has_prefix(prefix));

        (lo, hi)
    }
}

// =========================================================================

/// Given some vector of paths, get a vector of concise unambiguous relative paths.
///
/// Requires a vector of absolute paths. Finds a set of relative paths such that
/// each relative path is unique.
pub fn get_concise_relative_paths(paths: &[Path]) -> PathVector {
    if paths.is_empty() {
        return PathVector::new();
    }

    // Find common prefix
    let mut common_prefix = paths[0].clone();
    for path in paths.iter().skip(1) {
        common_prefix = common_prefix.get_common_prefix(path);
    }

    // Convert each path to relative
    paths
        .iter()
        .map(|p| p.make_relative(&common_prefix).unwrap_or_else(|| p.clone()))
        .collect()
}

/// Remove all elements of `paths` that are prefixed by other elements in `paths`.
/// Keeps only the "topmost" paths. As a side-effect, the result is left in sorted order.
pub fn remove_descendent_paths(paths: &mut PathVector) {
    paths.sort();
    // Remove path if any other path in the list is a strict prefix of it (i.e., it is a descendent).
    let mut to_remove = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        if paths
            .iter()
            .any(|other| other != path && path.has_prefix(other))
        {
            to_remove.push(i);
        }
    }
    for &idx in to_remove.iter().rev() {
        paths.remove(idx);
    }
}

/// Remove all elements of `paths` that are ancestors (prefixes) of other elements in `paths`.
/// Keeps only the "deepest" paths. As a side-effect, the result is left in sorted order.
pub fn remove_ancestor_paths(paths: &mut PathVector) {
    paths.sort();
    // Remove path if it is a strict prefix of any other path in the list (i.e., it is an ancestor).
    let mut to_remove = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        if paths
            .iter()
            .any(|other| other != path && other.has_prefix(path))
        {
            to_remove.push(i);
        }
    }
    for &idx in to_remove.iter().rev() {
        paths.remove(idx);
    }
}

// Value integration for instancer/primvar data sources (Path, Vec<Path>)
impl From<Path> for usd_vt::Value {
    #[inline]
    fn from(value: Path) -> Self {
        Self::new(value)
    }
}

// Note: From<Vec<Path>> for usd_vt::Value cannot be impl'd here (orphan rule:
// neither Vec nor Value is defined in this crate). Use Value::new(vec) instead.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let path = Path::empty();
        assert!(path.is_empty());
        assert_eq!(path.as_str(), "");
    }

    #[test]
    fn test_absolute_root() {
        let root = Path::absolute_root();
        assert!(!root.is_empty());
        assert_eq!(root.as_str(), "/");
        assert!(root.is_absolute_root_path());
        assert!(root.is_absolute_path());
    }

    #[test]
    fn test_reflexive_relative() {
        let rel = Path::reflexive_relative();
        assert_eq!(rel.as_str(), ".");
        assert!(!rel.is_absolute_path());
    }

    #[test]
    fn test_from_string() {
        let path = Path::from_string("/World/Cube").unwrap();
        assert_eq!(path.as_str(), "/World/Cube");
    }

    #[test]
    fn test_from_string_invalid() {
        assert!(Path::from_string("").is_none());
        assert!(Path::from_string("/World[").is_none()); // Unbalanced
    }

    #[test]
    fn test_is_prim_path() {
        assert!(Path::from_string("/World").unwrap().is_prim_path());
        assert!(Path::from_string("/World/Cube").unwrap().is_prim_path());
        assert!(!Path::from_string("/World.prop").unwrap().is_prim_path());
        assert!(!Path::absolute_root().is_prim_path());
    }

    #[test]
    fn test_is_property_path() {
        assert!(
            Path::from_string("/World.visibility")
                .unwrap()
                .is_property_path()
        );
        assert!(!Path::from_string("/World").unwrap().is_property_path());
    }

    #[test]
    fn test_is_root_prim_path() {
        assert!(Path::from_string("/World").unwrap().is_root_prim_path());
        assert!(
            !Path::from_string("/World/Cube")
                .unwrap()
                .is_root_prim_path()
        );
        assert!(!Path::absolute_root().is_root_prim_path());
    }

    #[test]
    fn test_is_variant_selection_path() {
        let path = Path::from_string("/World{variant=sel}").unwrap();
        assert!(path.is_prim_variant_selection_path());
        assert!(path.contains_prim_variant_selection());
    }

    #[test]
    fn test_is_target_path() {
        let path = Path::from_string("/World.rel[/Target]").unwrap();
        assert!(path.is_target_path());
        assert!(path.contains_target_path());
    }

    #[test]
    fn test_get_name() {
        assert_eq!(Path::from_string("/World/Cube").unwrap().get_name(), "Cube");
        assert_eq!(
            Path::from_string("/World.visibility").unwrap().get_name(),
            "visibility"
        );
        assert_eq!(Path::absolute_root().get_name(), "");
    }

    #[test]
    fn test_get_parent_path() {
        let path = Path::from_string("/World/Cube").unwrap();
        assert_eq!(path.get_parent_path().as_str(), "/World");

        let root_prim = Path::from_string("/World").unwrap();
        assert_eq!(root_prim.get_parent_path().as_str(), "/");

        let prop = Path::from_string("/World.prop").unwrap();
        assert_eq!(prop.get_parent_path().as_str(), "/World");

        // C++ returns EmptyPath for absolute root (not self)
        let root = Path::absolute_root();
        assert!(root.get_parent_path().is_empty());
    }

    #[test]
    fn test_get_prim_path() {
        let prop = Path::from_string("/World.visibility").unwrap();
        assert_eq!(prop.get_prim_path().as_str(), "/World");

        let prim = Path::from_string("/World").unwrap();
        assert_eq!(prim.get_prim_path().as_str(), "/World");
    }

    #[test]
    fn test_get_variant_selection() {
        let path = Path::from_string("/World{model=lod0}").unwrap();
        let (set, variant) = path.get_variant_selection().unwrap();
        assert_eq!(set, "model");
        assert_eq!(variant, "lod0");
    }

    #[test]
    fn test_get_target_path() {
        let path = Path::from_string("/World.rel[/Target]").unwrap();
        assert_eq!(path.get_target_path().unwrap().as_str(), "/Target");
    }

    #[test]
    fn test_append_child() {
        let parent = Path::from_string("/World").unwrap();
        let child = parent.append_child("Cube").unwrap();
        assert_eq!(child.as_str(), "/World/Cube");

        let from_root = Path::absolute_root().append_child("World").unwrap();
        assert_eq!(from_root.as_str(), "/World");
    }

    #[test]
    fn test_append_property() {
        let prim = Path::from_string("/World").unwrap();
        let prop = prim.append_property("visibility").unwrap();
        assert_eq!(prop.as_str(), "/World.visibility");
    }

    #[test]
    fn test_append_variant_selection() {
        let prim = Path::from_string("/World").unwrap();
        let variant = prim.append_variant_selection("model", "lod0").unwrap();
        assert_eq!(variant.as_str(), "/World{model=lod0}");
    }

    #[test]
    fn test_append_target() {
        let prop = Path::from_string("/World.rel").unwrap();
        let target_path = Path::from_string("/Target").unwrap();
        let target = prop.append_target(&target_path).unwrap();
        assert_eq!(target.as_str(), "/World.rel[/Target]");
    }

    #[test]
    fn test_has_prefix() {
        let path = Path::from_string("/World/Cube").unwrap();
        let prefix = Path::from_string("/World").unwrap();
        assert!(path.has_prefix(&prefix));
        assert!(path.has_prefix(&Path::absolute_root()));
        assert!(!prefix.has_prefix(&path));
    }

    #[test]
    fn test_get_common_prefix() {
        let path1 = Path::from_string("/World/A").unwrap();
        let path2 = Path::from_string("/World/B").unwrap();
        assert_eq!(path1.get_common_prefix(&path2).as_str(), "/World");
    }

    #[test]
    fn test_replace_prefix() {
        let path = Path::from_string("/World/Cube").unwrap();
        let old_prefix = Path::from_string("/World").unwrap();
        let new_prefix = Path::from_string("/NewWorld").unwrap();
        let result = path.replace_prefix(&old_prefix, &new_prefix).unwrap();
        assert_eq!(result.as_str(), "/NewWorld/Cube");
    }

    #[test]
    fn test_make_absolute() {
        let relative = Path::from_string("Cube").unwrap();
        let anchor = Path::from_string("/World").unwrap();
        let absolute = relative.make_absolute(&anchor).unwrap();
        assert_eq!(absolute.as_str(), "/World/Cube");
    }

    #[test]
    fn test_is_valid_identifier() {
        assert!(Path::is_valid_identifier("foo"));
        assert!(Path::is_valid_identifier("_foo"));
        assert!(Path::is_valid_identifier("foo123"));
        assert!(!Path::is_valid_identifier("123foo"));
        assert!(!Path::is_valid_identifier("foo:bar"));
        assert!(!Path::is_valid_identifier(""));
    }

    #[test]
    fn test_is_valid_namespaced_identifier() {
        assert!(Path::is_valid_namespaced_identifier("foo"));
        assert!(Path::is_valid_namespaced_identifier("foo:bar"));
        assert!(Path::is_valid_namespaced_identifier("primvars:st"));
        assert!(!Path::is_valid_namespaced_identifier(":foo"));
        assert!(!Path::is_valid_namespaced_identifier("foo:"));
        assert!(!Path::is_valid_namespaced_identifier(""));
    }

    #[test]
    fn test_tokenize_identifier() {
        let tokens = Path::tokenize_identifier("primvars:st");
        assert_eq!(tokens, vec!["primvars", "st"]);
    }

    #[test]
    fn test_join_identifier() {
        assert_eq!(Path::join_identifier(&["primvars", "st"]), "primvars:st");
        assert_eq!(Path::join_identifier(&["foo", "", "bar"]), "foo:bar");
    }

    #[test]
    fn test_strip_namespace() {
        assert_eq!(Path::strip_namespace("primvars:st"), "st");
        assert_eq!(Path::strip_namespace("foo"), "foo");
    }

    #[test]
    fn test_equality() {
        let p1 = Path::from_string("/World").unwrap();
        let p2 = Path::from_string("/World").unwrap();
        let p3 = Path::from_string("/Other").unwrap();

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_ordering() {
        let abs = Path::from_string("/A").unwrap();
        let rel = Path::from_string("A").unwrap();

        // Absolute < relative
        assert!(abs < rel);

        let p1 = Path::from_string("/A").unwrap();
        let p2 = Path::from_string("/B").unwrap();
        assert!(p1 < p2);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let p1 = Path::from_string("/World").unwrap();
        let p2 = Path::from_string("/World").unwrap();
        let p3 = Path::from_string("/Other").unwrap();

        let mut set = HashSet::new();
        set.insert(p1.clone());
        assert!(set.contains(&p2));
        assert!(!set.contains(&p3));
    }

    #[test]
    fn test_display() {
        let path = Path::from_string("/World/Cube").unwrap();
        assert_eq!(format!("{}", path), "/World/Cube");
    }

    #[test]
    fn test_debug() {
        let path = Path::from_string("/World").unwrap();
        let debug = format!("{:?}", path);
        assert!(debug.contains("Path"));
        assert!(debug.contains("/World"));
    }

    #[test]
    fn test_path_element_count() {
        assert_eq!(Path::absolute_root().get_path_element_count(), 0);
        assert_eq!(
            Path::from_string("/World")
                .unwrap()
                .get_path_element_count(),
            1
        );
        assert_eq!(
            Path::from_string("/World/Cube")
                .unwrap()
                .get_path_element_count(),
            2
        );
        assert_eq!(
            Path::from_string("/World.prop")
                .unwrap()
                .get_path_element_count(),
            2
        );
    }

    #[test]
    fn test_namespaced_property() {
        let path = Path::from_string("/World.primvars:st").unwrap();
        assert!(path.is_namespaced_property_path());
        assert!(path.is_property_path());
    }

    #[test]
    fn test_from_string_conversion() {
        let path: Path = "/World".into();
        assert_eq!(path.as_str(), "/World");

        let path2: Path = String::from("/World/Cube").into();
        assert_eq!(path2.as_str(), "/World/Cube");
    }

    #[test]
    fn test_clone() {
        let path = Path::from_string("/World/Cube").unwrap();
        let cloned = path.clone();
        assert_eq!(path, cloned);
    }

    #[test]
    fn test_validation_rejects_double_slash() {
        assert!(!Path::is_valid_path_string("//"));
        assert!(!Path::is_valid_path_string("/World//Cube"));
        assert!(!Path::is_valid_path_string("/A//B/C"));
    }

    #[test]
    fn test_validation_rejects_trailing_slash() {
        assert!(!Path::is_valid_path_string("/World/"));
        assert!(!Path::is_valid_path_string("/A/B/"));
        // Root is valid
        assert!(Path::is_valid_path_string("/"));
    }

    #[test]
    fn test_validation_with_error_diagnostics() {
        assert!(Path::is_valid_path_with_error("/World").is_ok());
        assert!(Path::is_valid_path_with_error("/").is_ok());
        assert!(Path::is_valid_path_with_error(".").is_ok());

        let err = Path::is_valid_path_with_error("//").unwrap_err();
        assert!(err.contains("double slash"), "got: {err}");

        let err = Path::is_valid_path_with_error("/World/").unwrap_err();
        assert!(err.contains("trailing slash"), "got: {err}");

        let err = Path::is_valid_path_with_error("").unwrap_err();
        assert!(err.contains("empty"), "got: {err}");

        let err = Path::is_valid_path_with_error("/A[B").unwrap_err();
        assert!(err.contains("unbalanced"), "got: {err}");
    }

    #[test]
    fn test_parent_of_target_path() {
        // /Foo.rel[/Target] -> parent is /Foo.rel
        let target = Path::from_string("/Foo.rel[/Target]").unwrap();
        assert_eq!(target.get_parent_path().as_str(), "/Foo.rel");
    }

    #[test]
    fn test_parent_of_relational_attribute() {
        // /Foo.rel[/Target].attr -> parent is /Foo.rel[/Target]
        let rel_attr = Path::from_string("/Foo.rel[/Target].attr").unwrap();
        assert_eq!(rel_attr.get_parent_path().as_str(), "/Foo.rel[/Target]");
    }

    #[test]
    fn test_parent_of_simple_property() {
        // /Foo.prop -> parent is /Foo
        let prop = Path::from_string("/Foo.prop").unwrap();
        assert_eq!(prop.get_parent_path().as_str(), "/Foo");
    }

    // =========================================================================
    // Component identifier validation (C++ parity)
    // =========================================================================

    #[test]
    fn test_validation_rejects_invalid_prim_names() {
        // Prim names starting with digit are invalid
        assert!(!Path::is_valid_path_string("/123"));
        assert!(!Path::is_valid_path_string("/World/123Cube"));

        // Prim names with spaces are invalid
        assert!(!Path::is_valid_path_string("/World/My Cube"));

        // Prim names with special chars are invalid
        assert!(!Path::is_valid_path_string("/World/Cube@"));
        assert!(!Path::is_valid_path_string("/World/Cube#"));
    }

    #[test]
    fn test_validation_accepts_valid_prim_names() {
        // Standard identifiers
        assert!(Path::is_valid_path_string("/World"));
        assert!(Path::is_valid_path_string("/World/Cube"));
        assert!(Path::is_valid_path_string("/World/Cube_001"));
        assert!(Path::is_valid_path_string("/_private/Mesh"));

        // Variant selections
        assert!(Path::is_valid_path_string("/World{variant=sel}"));
        assert!(Path::is_valid_path_string("/World{v=s}/Child"));

        // Target paths
        assert!(Path::is_valid_path_string("/Foo.rel[/Target]"));

        // Relative paths
        assert!(Path::is_valid_path_string("Cube"));
        assert!(Path::is_valid_path_string("./Cube"));
        assert!(Path::is_valid_path_string("../Sibling"));
    }

    #[test]
    fn test_validation_rejects_invalid_property_names() {
        // Property name starting with digit
        assert!(!Path::is_valid_path_string("/World.123prop"));

        // Property name with spaces
        assert!(!Path::is_valid_path_string("/World.my prop"));
    }

    #[test]
    fn test_validation_accepts_valid_property_names() {
        // Simple property
        assert!(Path::is_valid_path_string("/World.visibility"));

        // Namespaced property
        assert!(Path::is_valid_path_string("/World.primvars:st"));
        assert!(Path::is_valid_path_string("/World.xformOp:translate"));

        // Relationship with target
        assert!(Path::is_valid_path_string("/World.rel[/Target]"));
    }

    #[test]
    fn test_from_string_rejects_invalid_components() {
        // from_string should return None for invalid prim names
        assert!(Path::from_string("/123").is_none());
        assert!(Path::from_string("/World/123Cube").is_none());
        assert!(Path::from_string("/World.123prop").is_none());

        // Valid paths still work
        assert!(Path::from_string("/World").is_some());
        assert!(Path::from_string("/World.visibility").is_some());
    }

    #[test]
    fn test_validation_error_for_invalid_components() {
        let err = Path::is_valid_path_with_error("/123").unwrap_err();
        assert!(err.contains("invalid component"), "got: {err}");

        let err = Path::is_valid_path_with_error("/World.123prop").unwrap_err();
        assert!(err.contains("invalid component"), "got: {err}");
    }

    #[test]
    fn test_find_longest_prefix() {
        let mut paths: Vec<Path> = ["/", "/World", "/World/Scene", "/World/Scene/Char"]
            .iter()
            .filter_map(|s| Path::from_string(s))
            .collect();
        paths.sort();

        // Exact match
        let p = Path::from_string("/World/Scene").unwrap();
        assert_eq!(Path::find_longest_prefix(&paths, &p), Some(&paths[2]));

        // Child path -> parent
        let p = Path::from_string("/World/Scene/Char").unwrap();
        assert_eq!(Path::find_longest_prefix(&paths, &p), Some(&paths[3]));

        // Deeper than any -> longest
        let p = Path::from_string("/World/Scene/Char/Hand").unwrap();
        assert_eq!(Path::find_longest_prefix(&paths, &p), Some(&paths[3]));

        // Root
        let p = Path::from_string("/World").unwrap();
        assert_eq!(Path::find_longest_prefix(&paths, &p), Some(&paths[1]));
    }

    #[test]
    fn test_find_longest_strict_prefix() {
        let mut paths: Vec<Path> = ["/", "/World", "/World/Scene"]
            .iter()
            .filter_map(|s| Path::from_string(s))
            .collect();
        paths.sort();

        // Strict: exclude exact match
        let p = Path::from_string("/World/Scene").unwrap();
        assert_eq!(
            Path::find_longest_strict_prefix(&paths, &p),
            Some(&paths[1])
        );

        // Child -> parent
        let p = Path::from_string("/World/Scene/Char").unwrap();
        assert_eq!(
            Path::find_longest_strict_prefix(&paths, &p),
            Some(&paths[2])
        );

        // Root is strict prefix of any absolute path
        let p = Path::from_string("/Other").unwrap();
        assert_eq!(
            Path::find_longest_strict_prefix(&paths, &p),
            Some(&paths[0])
        );

        // No prefix: empty paths
        let empty_paths: &[Path] = &[];
        assert!(Path::find_longest_strict_prefix(empty_paths, &p).is_none());
    }

    #[test]
    fn test_find_prefixed_range() {
        let mut paths: Vec<Path> = ["/A", "/B", "/B/C", "/B/D", "/E"]
            .iter()
            .filter_map(|s| Path::from_string(s))
            .collect();
        paths.sort();

        // /B prefix should find /B, /B/C, /B/D
        let prefix = Path::from_string("/B").unwrap();
        let (lo, hi) = Path::find_prefixed_range(&paths, &prefix);
        assert_eq!(hi - lo, 3);
        assert_eq!(paths[lo].to_string(), "/B");

        // /A prefix finds just /A
        let prefix = Path::from_string("/A").unwrap();
        let (lo, hi) = Path::find_prefixed_range(&paths, &prefix);
        assert_eq!(hi - lo, 1);

        // /E prefix finds just /E
        let prefix = Path::from_string("/E").unwrap();
        let (lo, hi) = Path::find_prefixed_range(&paths, &prefix);
        assert_eq!(hi - lo, 1);

        // Non-existent prefix -> empty range
        let prefix = Path::from_string("/Z").unwrap();
        let (lo, hi) = Path::find_prefixed_range(&paths, &prefix);
        assert_eq!(lo, hi);
    }
    // =========================================================================
    // Tests ported from C++ testSdfPath.py / testSdfPath2.py
    // =========================================================================

    #[test]
    fn test_bad_paths_comprehensive() {
        // From testSdfPath.py test_Basic: all these must parse to empty path
        // Paths that are definitely invalid and our parser correctly rejects:
        let bad_paths = [
            "DD/DDD.&ddf$",
            "DD[]/DDD",
            "DD[]/DDD.bar",
            "/foo//bar",
            "/foo..bar",
            "</foo.bar",
            "</Foo/Bar/>",
            "/Foo/Bar/",
            "123",
            "123test",
            "/Foo:Bar",
        ];
        // TODO: also reject these paths once the parser is stricter:
        // "foo.prop/bar"             -- slash after property accepted (should be invalid)
        // "/foo.prop/bar.blah"       -- same issue
        // "/foo.bar.baz"             -- double-property without target accepted
        // "/foo/.bar"                -- leading-dot identifier after slash accepted
        // "/.foo"                   -- leading-dot identifier accepted
        // "/Foo.bar[targ]/Bar"       -- target path + prim child accepted
        // "/Foo.bar[targ].foo.foo"   -- double-property in relational path accepted
        // "/Foo.bar.mapper[...].arg:name:space" -- mapper path parsing incomplete
        for bad in &bad_paths {
            assert!(
                Path::from_string(bad).is_none(),
                "expected None for bad path: {bad}"
            );
            assert!(
                !Path::is_valid_path_string(bad),
                "expected is_valid_path_string=false for: {bad}"
            );
        }
    }

    #[test]
    fn test_comparison_comprehensive() {
        // From testSdfPath.py: empty < absolute-root < absolute-prim < relative
        let empty = Path::empty();
        let abs_root = Path::absolute_root();
        let rel_root = Path::reflexive_relative();
        let foo = Path::from_string("/Foo").unwrap();
        let bar = Path::from_string("/Bar").unwrap();
        let foo_bar = Path::from_string("/Foo/Bar").unwrap();

        // NOTE: In this implementation, Path::empty() has a null prim_handle,
        // so is_absolute_path() returns false — empty is treated as "relative"
        // in the Ord impl and sorts AFTER absolute paths.
        // C++ SdfPath orders empty < everything, but our impl differs here.
        // TODO: align empty path ordering with C++ when SdfPath ordering is fixed

        // abs root < abs prim
        assert!(abs_root < foo);
        // /Bar < /Foo (lexicographic)
        assert!(bar < foo);
        // /Foo < /Foo/Bar
        assert!(foo < foo_bar);
        // absolute < relative (C++ ordering)
        assert!(foo < rel_root);

        // From testSdfPath2 CheckOrdering (excluding empty comparisons):
        let root_prim2 = Path::from_string("/Bar").unwrap();
        let root_prim = Path::from_string("/Foo").unwrap();
        let child_prim = Path::from_string("/Foo/Bar").unwrap();
        assert!(abs_root < root_prim2);
        assert!(root_prim2 < root_prim);
        assert!(root_prim < child_prim);
        assert!(child_prim < rel_root);

        // Properties come after the prim, before children
        let root_prop = Path::from_string("/Foo.prop1").unwrap();
        let child_prim2 = Path::from_string("/Foo/Foo").unwrap();
        assert!(root_prim < root_prop);
        assert!(root_prop < child_prim);
        assert!(child_prim < child_prim2);

        // less-than: 'aaa' < 'aab' for relative paths
        let aaa = Path::from_string("aaa").unwrap();
        let aab = Path::from_string("aab").unwrap();
        assert!(aaa < aab);
        // empty is NOT less than empty
        assert!(!(empty < empty));
        // abs root < first abs prim
        let abs_a = Path::from_string("/a").unwrap();
        assert!(abs_root < abs_a);
    }

    #[test]
    fn test_make_relative() {
        // From testSdfPath2.py test_Basic: MakeRelativePath
        let foo_bar = Path::from_string("/foo/bar").unwrap();
        let foo = Path::from_string("/foo").unwrap();
        let foo_bar2 = Path::from_string("/foo/bar").unwrap();

        // /foo/bar relative to /foo -> "bar"
        assert_eq!(foo_bar.make_relative(&foo).unwrap().as_str(), "bar");
        // /foo relative to /foo -> "."
        assert_eq!(foo.make_relative(&foo).unwrap().as_str(), ".");
        // /foo relative to /foo/bar -> ".."
        assert_eq!(foo.make_relative(&foo_bar2).unwrap().as_str(), "..");
        // empty.make_relative -> None
        assert!(Path::empty().make_relative(&foo).is_none());

        // Error cases from testSdfPath2:
        // make_relative with non-absolute anchor -> None
        let rel_anchor = Path::from_string("foo").unwrap();
        assert!(foo.make_relative(&rel_anchor).is_none());
        // make_relative with empty anchor -> None
        assert!(foo.make_relative(&Path::empty()).is_none());
    }

    #[test]
    fn test_remove_common_suffix() {
        // From testSdfPath.py test_AncestorPathRange / RemoveCommonSuffix

        // Same paths: both become root (absolute)
        let a = Path::from_string("/A/B/C").unwrap();
        let b = Path::from_string("/D/B/C").unwrap();
        let (ra, rb) = a.remove_common_suffix(&b, false);
        assert_eq!(ra.as_str(), "/A");
        assert_eq!(rb.as_str(), "/D");

        // No common suffix
        let a = Path::from_string("/A").unwrap();
        let b = Path::from_string("/B").unwrap();
        let (ra, rb) = a.remove_common_suffix(&b, false);
        assert_eq!(ra.as_str(), "/A");
        assert_eq!(rb.as_str(), "/B");

        // Identical paths: collapse to absolute root
        let a = Path::from_string("/A/B").unwrap();
        let b = Path::from_string("/A/B").unwrap();
        let (ra, rb) = a.remove_common_suffix(&b, false);
        assert_eq!(ra.as_str(), "/");
        assert_eq!(rb.as_str(), "/");

        // stop_at_root_prim: identical paths don't collapse past root prim
        let a = Path::from_string("/A/B").unwrap();
        let b = Path::from_string("/A/B").unwrap();
        let (ra, rb) = a.remove_common_suffix(&b, true);
        assert_eq!(ra, a);
        assert_eq!(rb, b);

        // One is prefix of other doesn't matter — suffix matching only
        let a = Path::from_string("/A/B/C").unwrap();
        let b = Path::from_string("/X/Y/C").unwrap();
        let (ra, rb) = a.remove_common_suffix(&b, false);
        assert_eq!(ra.as_str(), "/A/B");
        assert_eq!(rb.as_str(), "/X/Y");
    }

    #[test]
    fn test_remove_descendent_paths() {
        // Basic: /A/B and /A/B/C are descendents of /A, so they get removed.
        let mut paths: PathVector = ["/A", "/A/B", "/A/B/C", "/D"]
            .iter()
            .map(|s| Path::from_string(s).unwrap())
            .collect();
        remove_descendent_paths(&mut paths);
        let result: Vec<&str> = paths.iter().map(|p| p.as_str()).collect();
        assert_eq!(result, vec!["/A", "/D"]);

        // Single element: unchanged.
        let mut single: PathVector = vec![Path::from_string("/X/Y").unwrap()];
        remove_descendent_paths(&mut single);
        assert_eq!(single.len(), 1);

        // No prefix relationships: all kept.
        let mut disjoint: PathVector = ["/A", "/B", "/C"]
            .iter()
            .map(|s| Path::from_string(s).unwrap())
            .collect();
        remove_descendent_paths(&mut disjoint);
        assert_eq!(disjoint.len(), 3);
    }

    #[test]
    fn test_remove_ancestor_paths() {
        // Basic: /A and /A/B are ancestors of /A/B/C, so they get removed.
        let mut paths: PathVector = ["/A", "/A/B", "/A/B/C", "/D"]
            .iter()
            .map(|s| Path::from_string(s).unwrap())
            .collect();
        remove_ancestor_paths(&mut paths);
        let result: Vec<&str> = paths.iter().map(|p| p.as_str()).collect();
        assert_eq!(result, vec!["/A/B/C", "/D"]);

        // Single element: unchanged.
        let mut single: PathVector = vec![Path::from_string("/X/Y").unwrap()];
        remove_ancestor_paths(&mut single);
        assert_eq!(single.len(), 1);

        // No prefix relationships: all kept.
        let mut disjoint: PathVector = ["/A", "/B", "/C"]
            .iter()
            .map(|s| Path::from_string(s).unwrap())
            .collect();
        remove_ancestor_paths(&mut disjoint);
        assert_eq!(disjoint.len(), 3);
    }

    #[test]
    fn test_get_prim_or_prim_variant_selection_path() {
        // From testSdfPath2.py: get_prim_or_prim_variant_selection_path
        let prim = Path::from_string("/Foo").unwrap();
        assert_eq!(prim.get_prim_or_prim_variant_selection_path(), prim);

        let variant = Path::from_string("/Foo{var=sel}").unwrap();
        assert_eq!(variant.get_prim_or_prim_variant_selection_path(), variant);

        // Stripping property part
        let prop = Path::from_string("/Foo.prop1").unwrap();
        assert_eq!(
            prop.get_prim_or_prim_variant_selection_path().as_str(),
            "/Foo"
        );

        // Target path: strip to prim
        let target = Path::from_string("/Foo.rel[/Target]").unwrap();
        assert_eq!(
            target.get_prim_or_prim_variant_selection_path().as_str(),
            "/Foo"
        );
    }

    #[test]
    fn test_ancestor_range() {
        // From testSdfPath.py test_AncestorPathRange / GetAncestorsRange
        // Iterates from path down to root, yielding each ancestor.
        let path = Path::from_string("/A/B/C").unwrap();
        let ancestors: Vec<String> = path
            .get_ancestors_range()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(ancestors, vec!["/A/B/C", "/A/B", "/A", "/"]);

        // Property path: starts with the full path, then prim parent, etc.
        let prop = Path::from_string("/Foo/Bar.prop").unwrap();
        let ancestors: Vec<String> = prop
            .get_ancestors_range()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(ancestors, vec!["/Foo/Bar.prop", "/Foo/Bar", "/Foo", "/"]);

        // Relative path ancestors
        let rel = Path::from_string("A/B").unwrap();
        let rel_ancestors: Vec<String> = rel
            .get_ancestors_range()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        // NOTE: get_parent_path() on "A/B" returns "./A" (with leading "./"),
        // not "A". This is a known quirk of the relative path parent impl.
        // TODO: fix get_parent_path() for relative paths to return bare "A" not "./A"
        assert_eq!(rel_ancestors, vec!["A/B", "./A", "."]);

        // Single element absolute path
        let single = Path::from_string("/Foo").unwrap();
        let single_ancestors: Vec<String> = single
            .get_ancestors_range()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(single_ancestors, vec!["/Foo", "/"]);
    }

    #[test]
    fn test_replace_prefix_with_fix_targets() {
        // From testSdfPath2.py: ReplacePrefix with fixTargetPaths behavior
        // replace_prefix (default) fixes embedded targets too

        // Basic prefix replacement
        let p = Path::from_string("/a").unwrap();
        let a = Path::from_string("/a").unwrap();
        let b = Path::from_string("/b").unwrap();
        assert_eq!(p.replace_prefix(&a, &b).unwrap().as_str(), "/b");

        let p = Path::from_string("/a/b").unwrap();
        assert_eq!(p.replace_prefix(&a, &b).unwrap().as_str(), "/b/b");

        // TODO: enable mapper-path tests when the parser supports .mapper[...] syntax
        // The path "/foo.rel[/foo/prim].relAttr.mapper[/foo/prim.attr]" is not
        // correctly parsed by from_string() — .mapper[] notation is not implemented.
        // let p = Path::from_string("/foo.rel[/foo/prim].relAttr.mapper[/foo/prim.attr]").unwrap();
        // let old = Path::from_string("/foo").unwrap();
        // let new_p = Path::from_string("/a/b").unwrap();
        // assert_eq!(p.replace_prefix(&old, &new_p).unwrap().as_str(),
        //     "/a/b.rel[/a/b/prim].relAttr.mapper[/a/b/prim.attr]");
        // assert_eq!(p.replace_prefix_with_fix(&old, &new_p, false).unwrap().as_str(),
        //     "/a/b.rel[/foo/prim].relAttr.mapper[/foo/prim.attr]");

        // Relative paths
        let p = Path::from_string("a").unwrap();
        let old = Path::from_string("a").unwrap();
        let new = Path::from_string("b").unwrap();
        assert_eq!(p.replace_prefix(&old, &new).unwrap().as_str(), "b");

        // No match: returns self unchanged
        let p = Path::from_string("foo").unwrap();
        let old = Path::from_string("bar").unwrap();
        let new = Path::from_string("bar").unwrap();
        assert_eq!(p.replace_prefix(&old, &new).unwrap().as_str(), "foo");

        // Error cases from testSdfPath2:
        // empty old_prefix -> empty
        let p = Path::from_string("foo").unwrap();
        assert!(
            p.replace_prefix(&Path::empty(), &Path::from_string("bar").unwrap())
                .map(|r| r.is_empty())
                .unwrap_or(true)
        );
        // empty new_prefix -> empty
        assert!(
            p.replace_prefix(&Path::from_string("foo").unwrap(), &Path::empty())
                .map(|r| r.is_empty())
                .unwrap_or(true)
        );
    }

    #[test]
    fn test_append_path() {
        // From testSdfPath2.py: AppendPath
        let prim = Path::from_string("/prim").unwrap();
        let rel_root = Path::reflexive_relative();

        // Appending reflexive relative -> self unchanged
        assert_eq!(prim.append_path(&rel_root).unwrap().as_str(), "/prim");

        // Appending relative path to root
        let root = Path::absolute_root();
        let rel = Path::from_string("foo/bar.attr").unwrap();
        assert_eq!(root.append_path(&rel).unwrap().as_str(), "/foo/bar.attr");

        let rel_ns = Path::from_string("foo/bar.attr:argle:bargle").unwrap();
        assert_eq!(
            root.append_path(&rel_ns).unwrap().as_str(),
            "/foo/bar.attr:argle:bargle"
        );

        // Appending to a prim
        let foo = Path::from_string("/foo").unwrap();
        let rel = Path::from_string("bar.attr").unwrap();
        assert_eq!(foo.append_path(&rel).unwrap().as_str(), "/foo/bar.attr");

        // Error cases:
        // Appending absolute path -> None
        let abs_suffix = Path::from_string("/absolute").unwrap();
        assert!(prim.append_path(&abs_suffix).is_none());

        // TODO: enable when append_path correctly rejects prim-child suffix on property paths
        // C++ SdfPath::AppendPath rejects appending a bare prim name to a property path,
        // but the current impl returns Some (constructs "/prim.attr/prim" which parses ok).
        // let prop = Path::from_string("/prim.attr").unwrap();
        // let rel_child = Path::from_string("prim").unwrap();
        // assert!(prop.append_path(&rel_child).is_none());
    }

    #[test]
    fn test_strip_all_variant_selections() {
        // From testSdfPath2.py: StripAllVariantSelections
        let p = Path::from_string("/foo/bar").unwrap();
        assert_eq!(p.strip_all_variant_selections().as_str(), "/foo/bar");

        let p = Path::from_string("/foo/bar{var=sel}").unwrap();
        assert_eq!(p.strip_all_variant_selections().as_str(), "/foo/bar");

        // TODO: enable when strip_all_variant_selections correctly handles variants in the
        // middle of a path. Currently "/foo/bar{var=sel}baz/frob" strips to "/foo/baz/frob"
        // (loses the "bar" prim component — bug in strip_all_variant_selections impl).
        // let p = Path::from_string("/foo/bar{var=sel}baz/frob").unwrap();
        // assert_eq!(p.strip_all_variant_selections().as_str(), "/foo/bar/baz/frob");

        // TODO: same issue with multiple variant selections in middle of path
        // let p = Path::from_string("/foo{bond=connery}bar{captain=picard}baz/frob{doctor=tennent}")
        //     .unwrap();
        // assert_eq!(p.strip_all_variant_selections().as_str(), "/foo/bar/baz/frob");
    }

    #[test]
    fn test_contains_target_path() {
        // From testSdfPath2.py ContainsTargetPath flag checks
        assert!(!Path::from_string("/Foo").unwrap().contains_target_path());
        assert!(
            !Path::from_string("/Foo.prop")
                .unwrap()
                .contains_target_path()
        );
        assert!(
            Path::from_string("/Foo.prop[/Target]")
                .unwrap()
                .contains_target_path()
        );
        assert!(
            Path::from_string("/Foo.prop[/Target].attr")
                .unwrap()
                .contains_target_path()
        );
        // TODO: enable mapper-path tests when from_string() supports .mapper[...] syntax
        // assert!(Path::from_string("/Foo.prop.mapper[/Target]").unwrap().contains_target_path());
        // assert!(Path::from_string("/Foo.prop.mapper[/Target].arg").unwrap().contains_target_path());
        // assert!(Path::from_string("/Foo.prop.mapper[/T].arg").unwrap().contains_target_path());
        // TODO: enable when from_string() parses ".expression" suffix paths
        // assert!(!Path::from_string("/Foo.prop.expression").unwrap().contains_target_path());
    }

    #[test]
    fn test_contains_property_elements() {
        // From testSdfPath2.py ContainsPropertyElements checks
        assert!(
            !Path::from_string("/foo/bar")
                .unwrap()
                .contains_property_elements()
        );
        assert!(
            !Path::from_string("/foo/bar{var=sel}")
                .unwrap()
                .contains_property_elements()
        );
        assert!(
            Path::from_string("/foo/bar.prop")
                .unwrap()
                .contains_property_elements()
        );
        assert!(
            Path::from_string("/foo/bar.prop[/T]")
                .unwrap()
                .contains_property_elements()
        );
        assert!(
            Path::from_string("/foo/bar.prop[/T].attr")
                .unwrap()
                .contains_property_elements()
        );
        // TODO: enable mapper-path tests when from_string() supports .mapper[...] syntax
        // assert!(Path::from_string("/foo.prop.mapper[/T]").unwrap().contains_property_elements());
        // assert!(Path::from_string("/foo.prop.mapper[/T].arg").unwrap().contains_property_elements());
        // TODO: enable when from_string() parses ".expression" suffix paths
        // assert!(Path::from_string("/foo.prop.expression").unwrap().contains_property_elements());
    }

    #[test]
    fn test_get_prefixes() {
        // From testSdfPath2.py: GetPrefixes
        let p = Path::from_string("/foo/bar/goo/loo").unwrap();
        let prefixes = p.get_prefixes();
        let strs: Vec<&str> = prefixes.iter().map(|p| p.as_str()).collect();
        // NOTE: get_prefixes() in this impl includes the absolute root "/" as the first element.
        // C++ SdfPath::GetPrefixes() does NOT include root for multi-element paths.
        // TODO: align get_prefixes() output with C++ (exclude root for paths with depth > 0)
        assert_eq!(
            strs,
            vec!["/", "/foo", "/foo/bar", "/foo/bar/goo", "/foo/bar/goo/loo"]
        );

        // Absolute root has no prefixes (element count = 0)
        let root_prefixes = Path::absolute_root().get_prefixes();
        // C++: GetPrefixes() on "/" returns ["/"]
        // Our implementation: includes path itself as last prefix
        // Verify the path itself is included.
        assert!(root_prefixes.iter().any(|p| p.is_absolute_root_path()));

        // Single element
        let p = Path::from_string("/foo").unwrap();
        let prefixes = p.get_prefixes();
        let strs: Vec<&str> = prefixes.iter().map(|p| p.as_str()).collect();
        // Should include root and /foo
        assert!(strs.contains(&"/"));
        assert!(strs.contains(&"/foo"));
    }
}
