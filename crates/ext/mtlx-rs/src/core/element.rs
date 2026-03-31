//! Element -- base class for MaterialX elements.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use indexmap::IndexMap;

use crate::core::types::NAME_PATH_SEPARATOR;
use crate::core::util;

/// Internal attribute key for tree ordering (not in C++ but used for optional ordering).
pub const TREE_ORDER_ATTRIBUTE: &str = "__tree_order";

// ---- ElementEquivalenceOptions ----

/// Floating-point format used when comparing values numerically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatFormat {
    /// Fixed decimal notation.
    Fixed,
    /// Scientific notation.
    Scientific,
    /// Default (shortest representation).
    Default,
}

impl Default for FloatFormat {
    fn default() -> Self {
        FloatFormat::Default
    }
}

/// Options controlling functional-equivalence comparison between elements.
/// Mirrors C++ `ElementEquivalenceOptions`.
#[derive(Debug, Clone)]
pub struct ElementEquivalenceOptions {
    /// When true, parse and re-format float values before comparing (tolerates precision diffs).
    /// Default: true.
    pub perform_value_comparisons: bool,
    /// Floating-point format to use when normalising float values.
    pub float_format: FloatFormat,
    /// Decimal digits of precision when normalising float values.
    pub float_precision: i32,
    /// Attribute names excluded from comparison.
    /// Name and category cannot be excluded.
    pub attribute_exclusion_list: HashSet<String>,
}

impl Default for ElementEquivalenceOptions {
    fn default() -> Self {
        Self {
            perform_value_comparisons: true,
            float_format: FloatFormat::Default,
            float_precision: 6,
            attribute_exclusion_list: HashSet::new(),
        }
    }
}

// Attribute names
pub const NAME_ATTRIBUTE: &str = "name";
pub const DOC_ATTRIBUTE: &str = "doc";
pub const TYPE_ATTRIBUTE: &str = "type";
pub const VALUE_ATTRIBUTE: &str = "value";
pub const FILE_PREFIX_ATTRIBUTE: &str = "fileprefix";
pub const GEOM_PREFIX_ATTRIBUTE: &str = "geomprefix";
pub const COLOR_SPACE_ATTRIBUTE: &str = "colorspace";
pub const INHERIT_ATTRIBUTE: &str = "inherit";
pub const TARGET_ATTRIBUTE: &str = "target";
pub const NODE_ATTRIBUTE: &str = "node";
pub const NODE_NAME_ATTRIBUTE: &str = "nodename";
pub const NODE_GRAPH_ATTRIBUTE: &str = "nodegraph";
pub const OUTPUT_ATTRIBUTE: &str = "output";
pub const DEFAULT_GEOM_PROP_ATTRIBUTE: &str = "defaultgeomprop";
pub const DEFAULT_ATTRIBUTE: &str = "default";
pub const XPOS_ATTRIBUTE: &str = "xpos";
pub const YPOS_ATTRIBUTE: &str = "ypos";
pub const WIDTH_ATTRIBUTE: &str = "width";
pub const HEIGHT_ATTRIBUTE: &str = "height";
pub const UNIFORM_ATTRIBUTE: &str = "uniform";
pub const PROPERTY_ATTRIBUTE: &str = "property";
pub const GEOM_ATTRIBUTE: &str = "geom";
pub const NODE_DEF_ATTRIBUTE: &str = "nodedef";
pub const VERSION_ATTRIBUTE: &str = "version";
pub const INTERFACE_NAME_ATTRIBUTE: &str = "interfacename";
pub const COLLECTION_ATTRIBUTE: &str = "collection";
pub const NAMESPACE_ATTRIBUTE: &str = "namespace";
pub const EXCLUDEGEOM_ATTRIBUTE: &str = "excludegeom";
pub const INCLUDEGEOM_ATTRIBUTE: &str = "includegeom";
pub const IMPLEMENTATION_NAME_ATTRIBUTE: &str = "implname";
pub const UNIT_ATTRIBUTE: &str = "unit";
pub const UNITTYPE_ATTRIBUTE: &str = "unittype";
pub const DEFAULT_VERSION_ATTRIBUTE: &str = "isdefaultversion";
pub const ENUM_ATTRIBUTE: &str = "enum";
pub const ENUM_VALUES_ATTRIBUTE: &str = "enumvalues";
pub const UI_NAME_ATTRIBUTE: &str = "uiname";
pub const UI_FOLDER_ATTRIBUTE: &str = "uifolder";
pub const UI_MIN_ATTRIBUTE: &str = "uimin";
pub const UI_MAX_ATTRIBUTE: &str = "uimax";
pub const UI_SOFT_MIN_ATTRIBUTE: &str = "uisoftmin";
pub const UI_SOFT_MAX_ATTRIBUTE: &str = "uisoftmax";
pub const UI_STEP_ATTRIBUTE: &str = "uistep";
pub const UI_ADVANCED_ATTRIBUTE: &str = "uiadvanced";
pub const HINT_ATTRIBUTE: &str = "hint";
pub const CMS_ATTRIBUTE: &str = "cms";
pub const CMS_CONFIG_ATTRIBUTE: &str = "cmsconfig";
pub const LOOKS_ATTRIBUTE: &str = "looks";
pub const ACTIVE_ATTRIBUTE: &str = "active";
pub const VARIANT_SET_ATTRIBUTE: &str = "variantset";
pub const VARIANT_ATTRIBUTE: &str = "variant";
pub const INCLUDE_COLLECTION_ATTRIBUTE: &str = "includecollection";

/// Element categories (MaterialX schema)
pub mod category {
    pub const DOCUMENT: &str = "materialx";
    pub const NODE_GRAPH: &str = "nodegraph";
    pub const NODE: &str = "node";
    pub const INPUT: &str = "input";
    pub const OUTPUT: &str = "output";
    pub const NODEDEF: &str = "nodedef";
    pub const IMPLEMENTATION: &str = "implementation";
    pub const TOKEN: &str = "token";
    pub const MATERIAL: &str = "material";
    pub const SHADER_REF: &str = "shaderref";
    pub const LOOK: &str = "look";
    pub const MATERIAL_ASSIGN: &str = "materialassign";
    pub const COMMENT: &str = "comment";
    pub const NEWLINE: &str = "newline";
    pub const TYPEDEF: &str = "typedef";
    pub const BACKDROP: &str = "backdrop";
    pub const GEOM_INFO: &str = "geominfo";
    pub const GEOM_PROP_DEF: &str = "geompropdef";
    pub const COLLECTION: &str = "collection";
    pub const PROPERTY_SET: &str = "propertyset";
    pub const PROPERTY: &str = "property";
    pub const PROPERTY_ASSIGN: &str = "propertyassign";
    pub const UNIT_TYPEDEF: &str = "unittypedef";
    pub const UNIT_DEF: &str = "unitdef";
    pub const UNIT: &str = "unit";
    pub const VARIANT_SET: &str = "variantset";
    pub const VARIANT: &str = "variant";
    pub const VISIBILITY: &str = "visibility";
    pub const LOOK_GROUP: &str = "lookgroup";
    pub const PROPERTY_SET_ASSIGN: &str = "propertysetassign";
    pub const VARIANT_ASSIGN: &str = "variantassign";
    pub const ATTRIBUTE_DEF: &str = "attributedef";
    pub const TARGET_DEF: &str = "targetdef";
    pub const GEOM_PROP: &str = "geomprop";
    pub const MEMBER: &str = "member";
}

/// Thread-safe shared pointer to Element (Arc<RwLock<Element>>).
///
/// Exposes `.borrow()` / `.borrow_mut()` for source-level compatibility
/// with existing callsites while using Arc+RwLock internally.
#[derive(Clone, Debug)]
pub struct ElementPtr(Arc<RwLock<Element>>);

/// Weak pointer to Element (for parent reference, avoids cycles).
#[derive(Clone, Debug)]
pub struct ElementWeakPtr(Weak<RwLock<Element>>);

impl ElementPtr {
    /// Create a new ElementPtr wrapping element.
    pub fn new(elem: Element) -> Self {
        Self(Arc::new(RwLock::new(elem)))
    }

    /// Shared read access — mirrors RefCell::borrow().
    pub fn borrow(&self) -> RwLockReadGuard<'_, Element> {
        self.0.read().expect("ElementPtr RwLock poisoned")
    }

    /// Exclusive write access — mirrors RefCell::borrow_mut().
    pub fn borrow_mut(&self) -> RwLockWriteGuard<'_, Element> {
        self.0.write().expect("ElementPtr RwLock poisoned")
    }

    /// Try shared read access without blocking.
    pub fn try_borrow(&self) -> Option<RwLockReadGuard<'_, Element>> {
        self.0.try_read().ok()
    }

    /// Returns a weak reference to this element.
    pub fn downgrade(&self) -> ElementWeakPtr {
        ElementWeakPtr(Arc::downgrade(&self.0))
    }

    /// Returns true if both point to the same element (pointer equality).
    pub fn ptr_eq(&self, other: &ElementPtr) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Raw pointer for use in HashSet-based cycle detection.
    pub fn as_raw_ptr(&self) -> *const RwLock<Element> {
        Arc::as_ptr(&self.0)
    }

    /// Rename this element and update the parent's child_map atomically.
    /// Returns Err if the name is invalid or already taken.
    pub fn rename(&self, new_name: impl Into<String>) -> Result<(), String> {
        let new_name = new_name.into();
        // Get old name and parent before acquiring write lock on self.
        let (old_name, parent_weak) = {
            let b = self.borrow();
            (b.name.clone(), b.parent.clone())
        };
        // Validate and set name (also checks sibling uniqueness).
        self.borrow_mut().set_name(&new_name)?;
        // Update parent's child_map: old_name -> new_name.
        if let Some(parent_weak) = parent_weak {
            if let Some(parent) = parent_weak.upgrade() {
                let mut pb = parent.borrow_mut();
                if let Some(idx) = pb.child_map.remove(&old_name) {
                    pb.child_map.insert(new_name, idx);
                }
            }
        }
        Ok(())
    }
}

impl ElementWeakPtr {
    /// Upgrade to a strong pointer, returning None if the element was dropped.
    pub fn upgrade(&self) -> Option<ElementPtr> {
        self.0.upgrade().map(ElementPtr)
    }
}

impl PartialEq for ElementPtr {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl Eq for ElementPtr {}

impl std::hash::Hash for ElementPtr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Use the raw pointer address as the hash (matches pointer-equality semantics).
        self.as_raw_ptr().hash(state);
    }
}

/// Base class for MaterialX elements.
/// Uses IndexMap for attributes to preserve insertion order (matches C++ _attributeOrder).
/// `child_map` mirrors C++ `_childMap` for O(1) `get_child` lookups.
#[derive(Debug)]
pub struct Element {
    pub(super) category: String,
    pub(super) name: String,
    pub(super) parent: Option<ElementWeakPtr>,
    pub(super) attributes: IndexMap<String, String>,
    pub(super) children: Vec<ElementPtr>,
    /// Name -> index into `children` for O(1) lookup (mirrors C++ _childMap).
    pub(super) child_map: HashMap<String, usize>,
    pub(super) source_uri: Option<String>,
}

impl Element {
    /// Create a new element. Internal use -- use Document::add_child_of_category instead.
    pub fn new(
        parent: Option<ElementWeakPtr>,
        category: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            category: category.into(),
            name: name.into(),
            parent,
            attributes: IndexMap::new(),
            children: Vec::new(),
            child_map: HashMap::new(),
            source_uri: None,
        }
    }

    pub fn get_category(&self) -> &str {
        &self.category
    }

    pub fn set_category(&mut self, category: impl Into<String>) {
        self.category = category.into();
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) -> Result<(), String> {
        let name = name.into();
        if !util::is_valid_name(&name) {
            return Err(format!("Invalid MaterialX name: '{}'", name));
        }
        // Check uniqueness among siblings. We avoid acquiring parent write lock
        // by using try_read on the parent (if it fails, skip -- caller already holds it).
        // For each sibling we use try_borrow; if it fails we compare raw pointers to
        // determine if the failure is because this IS self (already write-locked by caller)
        // or because another thread holds a write lock. Only the self case is safe to skip.
        if let Some(ref parent_weak) = self.parent {
            if let Some(parent) = parent_weak.upgrade() {
                // Try to acquire a read lock; if parent is already write-locked by caller,
                // skip the uniqueness check (caller is responsible in that case).
                if let Ok(parent_borrow) = parent.0.try_read() {
                    for sibling in &parent_borrow.children {
                        match sibling.0.try_read() {
                            Ok(sibling_borrow) => {
                                if sibling_borrow.name == name {
                                    return Err(format!(
                                        "Element name is not unique at the given scope: {}",
                                        name
                                    ));
                                }
                            }
                            Err(_) => {
                                // A write-locked sibling must be self -- any other write-locked
                                // sibling in a properly structured tree is a programming error.
                                // Skip safely.
                            }
                        }
                    }
                }
            }
        }
        self.name = name;
        Ok(())
    }

    pub fn get_parent(&self) -> Option<ElementPtr> {
        self.parent.as_ref().and_then(|w| w.upgrade())
    }

    pub fn get_children(&self) -> &[ElementPtr] {
        &self.children
    }

    /// O(1) child lookup via child_map (mirrors C++ _childMap).
    pub fn get_child(&self, name: &str) -> Option<ElementPtr> {
        self.child_map
            .get(name)
            .and_then(|&idx| self.children.get(idx))
            .cloned()
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.contains_key(name)
    }

    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(|s| s.as_str())
    }

    /// Set attribute, preserving insertion order (new keys appended, existing updated in-place).
    pub fn set_attribute(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(name.into(), value.into());
    }

    pub fn remove_attribute(&mut self, name: &str) {
        self.attributes.shift_remove(name);
    }

    /// Get attribute value or empty string (matches C++ getAttribute)
    pub fn get_attribute_or_empty(&self, name: &str) -> String {
        self.attributes.get(name).cloned().unwrap_or_default()
    }

    /// Get unit attribute (ValueElement)
    pub fn get_unit(&self) -> Option<&str> {
        self.attributes.get("unit").map(|s| s.as_str())
    }

    /// Get unittype attribute (ValueElement)
    pub fn get_unit_type(&self) -> Option<&str> {
        self.attributes.get("unittype").map(|s| s.as_str())
    }

    /// TypedElement: type attribute
    pub fn get_type(&self) -> Option<&str> {
        self.attributes.get(TYPE_ATTRIBUTE).map(|s| s.as_str())
    }
    pub fn set_type(&mut self, t: impl Into<String>) {
        self.attributes.insert(TYPE_ATTRIBUTE.to_string(), t.into());
    }
    pub fn has_type(&self) -> bool {
        self.has_attribute(TYPE_ATTRIBUTE)
    }

    /// ValueElement: value attribute
    pub fn get_value(&self) -> Option<&str> {
        self.attributes.get(VALUE_ATTRIBUTE).map(|s| s.as_str())
    }
    pub fn set_value(&mut self, v: impl Into<String>) {
        self.attributes
            .insert(VALUE_ATTRIBUTE.to_string(), v.into());
    }
    pub fn has_value(&self) -> bool {
        self.has_attribute(VALUE_ATTRIBUTE)
    }

    /// PortElement: nodename (connection to node)
    pub fn get_node_name(&self) -> Option<&str> {
        self.attributes.get(NODE_NAME_ATTRIBUTE).map(|s| s.as_str())
    }
    pub fn set_node_name(&mut self, name: impl Into<String>) {
        self.attributes
            .insert(NODE_NAME_ATTRIBUTE.to_string(), name.into());
    }
    pub fn has_node_name(&self) -> bool {
        self.has_attribute(NODE_NAME_ATTRIBUTE)
    }

    /// PortElement: nodegraph (connection to nodegraph)
    pub fn get_node_graph_string(&self) -> Option<&str> {
        self.attributes
            .get(NODE_GRAPH_ATTRIBUTE)
            .map(|s| s.as_str())
    }
    pub fn set_node_graph_string(&mut self, s: impl Into<String>) {
        self.attributes
            .insert(NODE_GRAPH_ATTRIBUTE.to_string(), s.into());
    }

    /// PortElement: output (which output of upstream)
    pub fn get_output_string(&self) -> Option<&str> {
        self.attributes.get(OUTPUT_ATTRIBUTE).map(|s| s.as_str())
    }
    pub fn set_output_string(&mut self, s: impl Into<String>) {
        self.attributes
            .insert(OUTPUT_ATTRIBUTE.to_string(), s.into());
    }

    /// Remove child by name, keeping child_map in sync.
    pub fn remove_child(&mut self, name: &str) {
        if let Some(idx) = self.child_map.remove(name) {
            self.children.remove(idx);
            // Reindex all entries after the removed position.
            for (_, v) in self.child_map.iter_mut() {
                if *v > idx {
                    *v -= 1;
                }
            }
        }
    }

    pub fn get_attribute_names(&self) -> impl Iterator<Item = &String> {
        self.attributes.keys()
    }

    /// Iterate over all attributes (key, value) in insertion order.
    pub fn iter_attributes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn get_source_uri(&self) -> Option<&str> {
        self.source_uri.as_deref()
    }

    pub fn set_source_uri(&mut self, uri: Option<impl Into<String>>) {
        self.source_uri = uri.map(Into::into);
    }

    /// Get source URI for XInclude tracking
    pub fn has_source_uri(&self) -> bool {
        self.source_uri.is_some()
    }

    /// File prefix attribute (for resolving relative paths)
    pub fn has_file_prefix(&self) -> bool {
        self.has_attribute(FILE_PREFIX_ATTRIBUTE)
    }
    pub fn get_file_prefix(&self) -> String {
        self.get_attribute_or_empty(FILE_PREFIX_ATTRIBUTE)
    }
    pub fn set_file_prefix(&mut self, prefix: impl Into<String>) {
        self.attributes
            .insert(FILE_PREFIX_ATTRIBUTE.to_string(), prefix.into());
    }

    // --- Inheritance ---

    /// Set the inherit string (element name this inherits from).
    pub fn set_inherit_string(&mut self, inherit: impl Into<String>) {
        self.attributes
            .insert(INHERIT_ATTRIBUTE.to_string(), inherit.into());
    }

    /// Return true if this element has an inherit string attribute.
    pub fn has_inherit_string(&self) -> bool {
        self.has_attribute(INHERIT_ATTRIBUTE)
    }

    /// Return the inherit string, or empty string.
    pub fn get_inherit_string(&self) -> String {
        self.get_attribute_or_empty(INHERIT_ATTRIBUTE)
    }

    /// Set the element this inherits from by its name path (None clears inheritance).
    pub fn set_inherits_from(&mut self, path: Option<&str>) {
        match path {
            Some(p) => self
                .attributes
                .insert(INHERIT_ATTRIBUTE.to_string(), p.to_string()),
            None => self.attributes.shift_remove(INHERIT_ATTRIBUTE),
        };
    }

    /// Return the inherited element path, if any.
    pub fn get_inherits_from(&self) -> Option<&str> {
        self.attributes.get(INHERIT_ATTRIBUTE).map(|s| s.as_str())
    }

    /// Return true if this element has an inheritance reference.
    pub fn has_inherits_from(&self) -> bool {
        self.has_attribute(INHERIT_ATTRIBUTE)
    }

    /// Geom prefix attribute
    pub fn has_geom_prefix(&self) -> bool {
        self.has_attribute(GEOM_PREFIX_ATTRIBUTE)
    }
    pub fn get_geom_prefix(&self) -> String {
        self.get_attribute_or_empty(GEOM_PREFIX_ATTRIBUTE)
    }
    pub fn set_geom_prefix(&mut self, prefix: impl Into<String>) {
        self.attributes
            .insert(GEOM_PREFIX_ATTRIBUTE.to_string(), prefix.into());
    }

    /// Color space attribute
    pub fn has_color_space(&self) -> bool {
        self.has_attribute(COLOR_SPACE_ATTRIBUTE)
    }
    pub fn get_color_space(&self) -> String {
        self.get_attribute_or_empty(COLOR_SPACE_ATTRIBUTE)
    }
    pub fn set_color_space(&mut self, cs: impl Into<String>) {
        self.attributes
            .insert(COLOR_SPACE_ATTRIBUTE.to_string(), cs.into());
    }

    /// Set namespace attribute
    pub fn set_namespace(&mut self, ns: impl Into<String>) {
        self.attributes
            .insert(NAMESPACE_ATTRIBUTE.to_string(), ns.into());
    }

    /// Has namespace attribute.
    pub fn has_namespace(&self) -> bool {
        self.has_attribute(NAMESPACE_ATTRIBUTE)
    }

    /// Get namespace attribute.
    pub fn get_namespace(&self) -> String {
        self.get_attribute_or_empty(NAMESPACE_ATTRIBUTE)
    }

    /// Qualified name (namespace:name or just name). Walks up to find namespace.
    pub fn get_qualified_name(&self, name: &str) -> String {
        use crate::core::types::NAME_PREFIX_SEPARATOR;
        let ns = self.get_attribute_or_empty(NAMESPACE_ATTRIBUTE);
        if !ns.is_empty() {
            if let Some(sep_pos) = name.find(NAME_PREFIX_SEPARATOR) {
                if name[..sep_pos] == ns {
                    return name.to_string();
                }
            }
            return format!("{}{}{}", ns, NAME_PREFIX_SEPARATOR, name);
        }
        if let Some(ref p) = self.parent {
            if let Some(parent) = p.upgrade() {
                return parent.borrow().get_qualified_name(name);
            }
        }
        name.to_string()
    }

    /// Get hierarchical name path (e.g. "nodegraph1/node1").
    pub fn get_name_path(&self, relative_to: Option<&ElementPtr>) -> String {
        let mut path = vec![self.name.clone()];
        let mut current = self.parent.clone();
        while let Some(weak) = current {
            if let Some(p) = weak.upgrade() {
                if let Some(rel) = relative_to {
                    if p.ptr_eq(rel) {
                        break;
                    }
                }
                if p.borrow().get_category() == category::DOCUMENT {
                    break;
                }
                path.push(p.borrow().name.clone());
                current = p.borrow().parent.clone();
            } else {
                break;
            }
        }
        path.reverse();
        path.into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(&NAME_PATH_SEPARATOR.to_string())
    }

    /// Add a child element, keeping child_map in sync.
    pub fn add_child(&mut self, child: ElementPtr) {
        let name = child.borrow().name.clone();
        let idx = self.children.len();
        self.children.push(child);
        self.child_map.insert(name, idx);
    }

    // --- Documentation ---

    /// Set documentation string (C++ Element::setDocString)
    pub fn set_doc_string(&mut self, doc: impl Into<String>) {
        self.attributes
            .insert(DOC_ATTRIBUTE.to_string(), doc.into());
    }

    /// Get documentation string
    pub fn get_doc_string(&self) -> String {
        self.get_attribute_or_empty(DOC_ATTRIBUTE)
    }

    // --- TypedElement helpers ---

    /// Return true if the element is of color type
    pub fn is_color_type(&self) -> bool {
        matches!(self.get_type(), Some("color3") | Some("color4"))
    }

    /// Return true if the element is of multi-output type
    pub fn is_multi_output_type(&self) -> bool {
        self.get_type() == Some(crate::core::types::MULTI_OUTPUT_TYPE_STRING)
    }

    // --- ValueElement helpers ---

    /// Set value string (C++ ValueElement::setValueString)
    pub fn set_value_string(&mut self, value: impl Into<String>) {
        self.attributes
            .insert(VALUE_ATTRIBUTE.to_string(), value.into());
    }

    /// Has value string
    pub fn has_value_string(&self) -> bool {
        self.has_attribute(VALUE_ATTRIBUTE)
    }

    /// Get value string
    pub fn get_value_string(&self) -> String {
        self.get_attribute_or_empty(VALUE_ATTRIBUTE)
    }

    /// Set interface name (C++ ValueElement::setInterfaceName)
    pub fn set_interface_name(&mut self, name: impl Into<String>) {
        self.attributes
            .insert(INTERFACE_NAME_ATTRIBUTE.to_string(), name.into());
    }

    /// Has interface name
    pub fn has_interface_name(&self) -> bool {
        self.has_attribute(INTERFACE_NAME_ATTRIBUTE)
    }

    /// Get interface name
    pub fn get_interface_name(&self) -> String {
        self.get_attribute_or_empty(INTERFACE_NAME_ATTRIBUTE)
    }

    /// Set implementation name (C++ ValueElement::setImplementationName)
    pub fn set_implementation_name(&mut self, name: impl Into<String>) {
        self.attributes
            .insert(IMPLEMENTATION_NAME_ATTRIBUTE.to_string(), name.into());
    }

    /// Has implementation name
    pub fn has_implementation_name(&self) -> bool {
        self.has_attribute(IMPLEMENTATION_NAME_ATTRIBUTE)
    }

    /// Get implementation name
    pub fn get_implementation_name(&self) -> String {
        self.get_attribute_or_empty(IMPLEMENTATION_NAME_ATTRIBUTE)
    }

    /// Set unit string (C++ ValueElement::setUnit)
    pub fn set_unit(&mut self, unit: impl Into<String>) {
        self.attributes
            .insert(UNIT_ATTRIBUTE.to_string(), unit.into());
    }

    /// Has unit string
    pub fn has_unit(&self) -> bool {
        self.has_attribute(UNIT_ATTRIBUTE)
    }

    /// Set unit type string (C++ ValueElement::setUnitType)
    pub fn set_unit_type(&mut self, unit_type: impl Into<String>) {
        self.attributes
            .insert(UNITTYPE_ATTRIBUTE.to_string(), unit_type.into());
    }

    /// Has unit type
    pub fn has_unit_type(&self) -> bool {
        self.has_attribute(UNITTYPE_ATTRIBUTE)
    }

    /// Set uniform flag (C++ ValueElement::setIsUniform)
    pub fn set_is_uniform(&mut self, value: bool) {
        self.attributes.insert(
            UNIFORM_ATTRIBUTE.to_string(),
            if value { "true" } else { "false" }.to_string(),
        );
    }

    /// Get uniform flag
    pub fn get_is_uniform(&self) -> bool {
        self.get_attribute(UNIFORM_ATTRIBUTE) == Some("true")
    }

    // --- Active-scope walks ---

    /// Get active file prefix (walk ancestors, C++ Element::getActiveFilePrefix)
    pub fn get_active_file_prefix(&self) -> String {
        if self.has_file_prefix() {
            return self.get_file_prefix();
        }
        if let Some(ref p) = self.parent {
            if let Some(parent) = p.upgrade() {
                return parent.borrow().get_active_file_prefix();
            }
        }
        String::new()
    }

    /// Get active geom prefix (walk ancestors)
    pub fn get_active_geom_prefix(&self) -> String {
        if self.has_geom_prefix() {
            return self.get_geom_prefix();
        }
        if let Some(ref p) = self.parent {
            if let Some(parent) = p.upgrade() {
                return parent.borrow().get_active_geom_prefix();
            }
        }
        String::new()
    }

    /// Get active source URI (walk ancestors, C++ Element::getActiveSourceUri)
    pub fn get_active_source_uri(&self) -> String {
        if self.source_uri.is_some() {
            return self.source_uri.clone().unwrap_or_default();
        }
        if let Some(ref p) = self.parent {
            if let Some(parent) = p.upgrade() {
                return parent.borrow().get_active_source_uri();
            }
        }
        String::new()
    }

    // --- Child helpers ---

    /// Get children of a specific category
    pub fn get_children_of_category(&self, cat: &str) -> Vec<ElementPtr> {
        self.children
            .iter()
            .filter(|c| c.borrow().category == cat)
            .cloned()
            .collect()
    }

    /// Get child by name and category
    pub fn get_child_of_category(&self, name: &str, cat: &str) -> Option<ElementPtr> {
        let child = self.get_child(name)?;
        if child.borrow().category == cat {
            Some(child)
        } else {
            None
        }
    }

    /// Set child index (reorder children). C++ Element::setChildIndex.
    pub fn set_child_index(&mut self, name: &str, index: usize) -> Result<(), String> {
        let pos = self.children.iter().position(|c| c.borrow().name == name);
        match pos {
            None => Err(format!("Child not found: {}", name)),
            Some(old_idx) => {
                if index >= self.children.len() {
                    return Err(format!("Index out of bounds: {}", index));
                }
                let child = self.children.remove(old_idx);
                self.children.insert(index, child);
                // Rebuild child_map since all indices may have shifted.
                self.rebuild_child_map();
                Ok(())
            }
        }
    }

    /// Rebuild child_map from the current children vec (O(n), used after reorders).
    fn rebuild_child_map(&mut self) {
        self.child_map.clear();
        for (idx, child) in self.children.iter().enumerate() {
            let name = child.borrow().name.clone();
            self.child_map.insert(name, idx);
        }
    }

    /// Get child index. Returns None if not found.
    pub fn get_child_index(&self, name: &str) -> Option<usize> {
        self.child_map.get(name).copied()
    }

    /// Remove child of specific category (C++ Element::removeChildOfType)
    pub fn remove_child_of_category(&mut self, name: &str, cat: &str) {
        if let Some(child) = self.get_child(name) {
            if child.borrow().category == cat {
                self.remove_child(name);
            }
        }
    }

    /// Create a valid unique child name (C++ Element::createValidChildName)
    pub fn create_valid_child_name(&self, name: &str) -> String {
        let mut n = if name.is_empty() {
            "_".to_string()
        } else {
            util::create_valid_name(name, '_')
        };
        while self.get_child(&n).is_some() {
            n = util::increment_name(&n);
        }
        n
    }

    /// Clear all content (C++ Element::clearContent)
    pub fn clear_content(&mut self) {
        self.attributes.clear();
        self.children.clear();
        self.source_uri = None;
    }

    /// Single-line description (C++ Element::asString)
    pub fn as_string(&self) -> String {
        let mut s = format!("{} name=\"{}\"", self.category, self.name);
        for (k, v) in &self.attributes {
            s.push_str(&format!(" {}=\"{}\"", k, v));
        }
        s
    }

    /// Get descendant by path (C++ Element::getDescendant)
    pub fn get_descendant(&self, name_path: &str) -> Option<ElementPtr> {
        let parts: Vec<&str> = name_path
            .split(NAME_PATH_SEPARATOR)
            .filter(|p| !p.is_empty())
            .collect();
        if parts.is_empty() {
            return None;
        }
        let mut current = self.get_child(parts[0])?;
        for part in &parts[1..] {
            let next = current.borrow().get_child(part)?;
            current = next;
        }
        Some(current)
    }

    // --- PortElement helpers ---

    /// Has nodegraph string
    pub fn has_node_graph_string(&self) -> bool {
        self.has_attribute(NODE_GRAPH_ATTRIBUTE)
    }

    /// Has output string
    pub fn has_output_string(&self) -> bool {
        self.has_attribute(OUTPUT_ATTRIBUTE)
    }

    // --- Target ---

    /// Set target attribute
    pub fn set_target(&mut self, target: impl Into<String>) {
        self.attributes
            .insert(TARGET_ATTRIBUTE.to_string(), target.into());
    }

    /// Has target attribute
    pub fn has_target(&self) -> bool {
        self.has_attribute(TARGET_ATTRIBUTE)
    }

    /// Get target attribute
    pub fn get_target(&self) -> String {
        self.get_attribute_or_empty(TARGET_ATTRIBUTE)
    }

    // --- Version ---

    /// Set version string (C++ InterfaceElement::setVersionString)
    pub fn set_version_string(&mut self, version: impl Into<String>) {
        self.attributes
            .insert(VERSION_ATTRIBUTE.to_string(), version.into());
    }

    /// Has version string
    pub fn has_version_string(&self) -> bool {
        self.has_attribute(VERSION_ATTRIBUTE)
    }

    /// Get version string
    pub fn get_version_string(&self) -> String {
        self.get_attribute_or_empty(VERSION_ATTRIBUTE)
    }

    /// Set default version flag (C++ InterfaceElement::setDefaultVersion)
    pub fn set_default_version(&mut self, is_default: bool) {
        self.attributes.insert(
            DEFAULT_VERSION_ATTRIBUTE.to_string(),
            if is_default { "true" } else { "false" }.to_string(),
        );
    }

    /// Get default version flag
    pub fn get_default_version(&self) -> bool {
        self.get_attribute(DEFAULT_VERSION_ATTRIBUTE) == Some("true")
    }

    /// Count input children
    pub fn get_input_count(&self) -> usize {
        self.children
            .iter()
            .filter(|c| c.borrow().category == category::INPUT)
            .count()
    }

    /// Count output children
    pub fn get_output_count(&self) -> usize {
        self.children
            .iter()
            .filter(|c| c.borrow().category == category::OUTPUT)
            .count()
    }
}

/// Copy attributes and children from source into dest (recursive). Skips name.
pub fn copy_content_from_element(dest: &ElementPtr, source: &Element) {
    {
        let mut dest_mut = dest.borrow_mut();
        for (k, v) in &source.attributes {
            if k != NAME_ATTRIBUTE {
                dest_mut.attributes.insert(k.clone(), v.clone());
            }
        }
        dest_mut.source_uri = source.source_uri.clone();
    }
    for child in source.get_children() {
        let src = child.borrow();
        let name = src.name.clone();
        let cat = src.category.clone();
        drop(src);
        if dest.borrow().get_child(&name).is_some() {
            continue;
        }
        let new_ptr = add_child_of_category(dest, &cat, &name)
            .expect("copy_content_from: add_child_of_category");
        copy_content_from_element(&new_ptr, &child.borrow());
    }
}

/// Add a child of the given category with the given name to the parent element.
pub fn add_child_of_category(
    parent: &ElementPtr,
    category: impl Into<String>,
    name: impl Into<String>,
) -> Result<ElementPtr, String> {
    let name = name.into();
    let category = category.into();
    let child_name = if name.is_empty() {
        // C++ Element::createValidChildName: start at "<prefix>1" and increment until unique.
        // Using count+1 as starting point would produce collisions after deletions.
        let parent_cat = parent.borrow().get_category().to_string();
        let prefix = if (parent_cat == category::NODE_GRAPH || parent_cat == category::DOCUMENT)
            && category != category::INPUT
            && category != category::OUTPUT
        {
            "node".to_string()
        } else {
            util::create_valid_name(&category, '_')
        };
        let mut index = 1usize;
        loop {
            let candidate = format!("{}{}", prefix, index);
            if parent.borrow().get_child(&candidate).is_none() {
                break candidate;
            }
            index += 1;
        }
    } else {
        if !util::is_valid_name(&name) {
            return Err(format!("Invalid name: '{}'", name));
        }
        name
    };
    if parent.borrow().get_child(&child_name).is_some() {
        return Err(format!("Element '{}' already exists", child_name));
    }
    let parent_weak = parent.downgrade();
    let child = Element::new(Some(parent_weak), &category, &child_name);
    let child_ptr = ElementPtr::new(child);
    parent.borrow_mut().add_child(child_ptr.clone());
    Ok(child_ptr)
}

/// Get the root (document) element from any element in the tree.
pub fn get_root(elem: &ElementPtr) -> ElementPtr {
    if let Some(p) = &elem.borrow().parent {
        if let Some(parent) = p.upgrade() {
            return get_root(&parent);
        }
    }
    elem.clone()
}

/// Return true if the inheritance chain for `elem` contains a cycle.
pub fn has_inheritance_cycle(elem: &ElementPtr, scope: &[ElementPtr]) -> bool {
    let max_depth = scope.len() + 2;
    let mut current_name = {
        let b = elem.borrow();
        match b.get_inherits_from() {
            Some(s) => s.to_string(),
            None => return false,
        }
    };
    let start_name = elem.borrow().get_name().to_string();
    for _ in 0..max_depth {
        if current_name == start_name {
            return true;
        }
        let next = scope.iter().find(|e| e.borrow().get_name() == current_name);
        match next {
            None => return false,
            Some(e) => {
                let b = e.borrow();
                match b.get_inherits_from() {
                    Some(s) => current_name = s.to_string(),
                    None => return false,
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_root(name: &str) -> ElementPtr {
        ElementPtr::new(Element::new(None, category::DOCUMENT, name))
    }

    fn add(parent: &ElementPtr, cat: &str, name: &str) -> ElementPtr {
        add_child_of_category(parent, cat, name).unwrap()
    }

    // -- tree_order --
    #[test]
    fn tree_order_roundtrip() {
        let root = make_root("doc");
        let child = add(&root, "input", "a");
        assert_eq!(get_tree_order(&child), None);
        set_tree_order(&child, 42);
        assert_eq!(get_tree_order(&child), Some(42));
        set_tree_order(&child, -1);
        assert_eq!(get_tree_order(&child), Some(-1));
    }

    // -- change_child_category --
    #[test]
    fn change_child_category_basic() {
        let root = make_root("doc");
        let child = add(&root, "input", "myport");
        child.borrow_mut().set_attribute("type", "float");
        child.borrow_mut().set_attribute("value", "1.0");

        let new_child = change_child_category(&root, "myport", "output")
            .expect("change_child_category returned None");

        // Category updated.
        assert_eq!(new_child.borrow().get_category(), "output");
        // Name preserved.
        assert_eq!(new_child.borrow().get_name(), "myport");
        // Attributes copied.
        assert_eq!(new_child.borrow().get_attribute("type"), Some("float"));
        assert_eq!(new_child.borrow().get_attribute("value"), Some("1.0"));
        // Position preserved -- should be at index 0.
        assert_eq!(root.borrow().get_child_index("myport"), Some(0));
    }

    #[test]
    fn change_child_category_preserves_order() {
        let root = make_root("doc");
        add(&root, "input", "first");
        add(&root, "input", "middle");
        add(&root, "input", "last");

        change_child_category(&root, "middle", "output").unwrap();
        // Should still be at index 1.
        assert_eq!(root.borrow().get_child_index("middle"), Some(1));
    }

    // -- resolve_name_reference --
    #[test]
    fn resolve_name_reference_finds_sibling() {
        let root = make_root("doc");
        let _nd = add(&root, "nodedef", "ND_foo");
        let node = add(&root, "node", "node1");

        let found = resolve_name_reference(&node, "ND_foo");
        assert!(found.is_some());
        assert_eq!(found.unwrap().borrow().get_name(), "ND_foo");
    }

    #[test]
    fn resolve_name_reference_missing() {
        let root = make_root("doc");
        let node = add(&root, "node", "n");
        assert!(resolve_name_reference(&node, "nonexistent").is_none());
    }

    // -- set/get_inherits_from + has_inherited_base --
    #[test]
    fn inherits_from_roundtrip() {
        let root = make_root("doc");
        let base = add(&root, "nodedef", "ND_base");
        let derived = add(&root, "nodedef", "ND_derived");

        // Should not inherit initially.
        assert!(get_inherits_from(&derived).is_none());

        set_inherits_from(&derived, Some(&base));
        assert_eq!(derived.borrow().get_inherit_string(), "ND_base");

        let resolved = get_inherits_from(&derived);
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ptr_eq(&base));

        // Clear inheritance.
        set_inherits_from(&derived, None);
        assert!(get_inherits_from(&derived).is_none());
    }

    #[test]
    fn has_inherited_base_chain() {
        let root = make_root("doc");
        let a = add(&root, "nodedef", "A");
        let b = add(&root, "nodedef", "B");
        let c = add(&root, "nodedef", "C");

        // B inherits A, C inherits B.
        set_inherits_from(&b, Some(&a));
        set_inherits_from(&c, Some(&b));

        assert!(has_inherited_base(&c, "B"));
        assert!(has_inherited_base(&c, "A"));
        assert!(!has_inherited_base(&c, "C")); // not self
        assert!(!has_inherited_base(&b, "C")); // not up the chain
    }

    // -- is_equivalent --
    #[test]
    fn is_equivalent_identical_elements() {
        let root_a = make_root("doc");
        let root_b = make_root("doc");
        let opts = ElementEquivalenceOptions::default();

        let a = add(&root_a, "input", "x");
        let b = add(&root_b, "input", "x");
        a.borrow_mut().set_attribute("type", "float");
        b.borrow_mut().set_attribute("type", "float");

        assert!(is_equivalent(&a, &b, &opts));
    }

    #[test]
    fn is_equivalent_different_name() {
        let root_a = make_root("doc");
        let root_b = make_root("doc");
        let opts = ElementEquivalenceOptions::default();
        let a = add(&root_a, "input", "x");
        let b = add(&root_b, "input", "y");
        assert!(!is_equivalent(&a, &b, &opts));
    }

    #[test]
    fn is_equivalent_float_tolerance() {
        let root_a = make_root("doc");
        let root_b = make_root("doc");
        let mut opts = ElementEquivalenceOptions::default();
        opts.float_precision = 3;

        let a = add(&root_a, "input", "v");
        let b = add(&root_b, "input", "v");
        // Same value to 3 decimal places but different string representation.
        a.borrow_mut().set_attribute(VALUE_ATTRIBUTE, "0.100");
        b.borrow_mut().set_attribute(VALUE_ATTRIBUTE, "0.1");
        a.borrow_mut().set_attribute("type", "float");
        b.borrow_mut().set_attribute("type", "float");

        assert!(is_equivalent(&a, &b, &opts));
    }

    #[test]
    fn is_equivalent_skips_comment_children() {
        let root_a = make_root("doc");
        let root_b = make_root("doc");
        let opts = ElementEquivalenceOptions::default();

        let a = add(&root_a, "nodegraph", "g");
        let b = add(&root_b, "nodegraph", "g");
        // Add a comment child only to `a` -- should still be equivalent.
        add(&a, category::COMMENT, "c1");

        assert!(is_equivalent(&a, &b, &opts));
    }

    #[test]
    fn is_equivalent_exclusion_list() {
        let root_a = make_root("doc");
        let root_b = make_root("doc");
        let mut opts = ElementEquivalenceOptions::default();
        opts.attribute_exclusion_list
            .insert(DOC_ATTRIBUTE.to_string());

        let a = add(&root_a, "input", "p");
        let b = add(&root_b, "input", "p");
        a.borrow_mut().set_attribute(DOC_ATTRIBUTE, "some docs");
        // b has no doc attribute -- but it's excluded so still equivalent.
        assert!(is_equivalent(&a, &b, &opts));
    }

    // -- is_attribute_equivalent --
    #[test]
    fn is_attribute_equivalent_float_normalise() {
        let root = make_root("doc");
        let a = add(&root, "input", "a");
        let b = add(&root, "input", "b");
        a.borrow_mut().set_attribute(VALUE_ATTRIBUTE, "1.000000");
        b.borrow_mut().set_attribute(VALUE_ATTRIBUTE, "1");
        let opts = ElementEquivalenceOptions {
            float_precision: 4,
            ..Default::default()
        };
        assert!(is_attribute_equivalent(&a, &b, VALUE_ATTRIBUTE, &opts));
    }

    #[test]
    fn is_attribute_equivalent_string_mismatch() {
        let root = make_root("doc");
        let a = add(&root, "input", "a");
        let b = add(&root, "input", "b");
        a.borrow_mut().set_attribute("type", "float");
        b.borrow_mut().set_attribute("type", "integer");
        let opts = ElementEquivalenceOptions::default();
        assert!(!is_attribute_equivalent(&a, &b, "type", &opts));
    }
}

// ---- newline element factory ----

/// Add a newline element as a child of `parent`.
/// Mirrors C++ Element::addChild<NewlineElement>.
/// Returns the new element (category = "newline", auto-generated name).
pub fn add_newline(parent: &ElementPtr) -> Result<ElementPtr, String> {
    let count = parent.borrow().get_children().len();
    let name = format!("newline{}", count + 1);
    add_child_of_category(parent, category::NEWLINE, &name)
}

// ---- ValueElement resolved value / default value / active unit ----

/// Return the resolved value string for a ValueElement, applying StringResolver substitutions.
/// Only resolves filename/geomname types; other types return the raw value string.
/// Mirrors C++ ValueElement::getResolvedValueString.
pub fn get_resolved_value_string(elem: &ElementPtr, geom: &str) -> String {
    let b = elem.borrow();
    let val = b.get_value_string();
    let type_str = b.get_type().unwrap_or("").to_string();
    drop(b);
    if !util::StringResolver::is_resolved_type(&type_str) {
        return val;
    }
    let resolver = create_string_resolver(elem, geom);
    resolver.resolve(&val, &type_str)
}

/// Return the default value string for a ValueElement by looking up the
/// declaration's active value element of the same name.
/// Mirrors C++ ValueElement::getDefaultValue.
pub fn get_default_value_string(elem: &ElementPtr) -> Option<String> {
    let parent = elem.borrow().get_parent()?;
    let decl = crate::core::interface::get_declaration(&parent)?;
    let elem_name = elem.borrow().get_name().to_string();
    let ve = crate::core::interface::get_active_value_element(&decl, &elem_name)?;
    let val = ve.borrow().get_value_string();
    if val.is_empty() { None } else { Some(val) }
}

/// Return the active unit string for a ValueElement. Looks up the unit on
/// the declaration's matching value element.
/// Mirrors C++ ValueElement::getActiveUnit.
pub fn get_active_unit(elem: &ElementPtr) -> String {
    let parent = match elem.borrow().get_parent() {
        Some(p) => p,
        None => return String::new(),
    };
    let decl = match crate::core::interface::get_declaration(&parent) {
        Some(d) => d,
        None => return String::new(),
    };
    let elem_name = elem.borrow().get_name().to_string();
    match crate::core::interface::get_active_value_element(&decl, &elem_name) {
        Some(ve) => ve.borrow().get_unit().unwrap_or("").to_string(),
        None => String::new(),
    }
}

// ---- validate ----

/// Validate a single element: check name validity and that inherit string resolves
/// to an element of the same category. Returns (valid, error_messages).
/// Mirrors C++ Element::validate (base-class portion only, no child recursion).
pub fn validate_element_self(elem: &ElementPtr) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut valid = true;

    // Name must be valid.
    let name = elem.borrow().get_name().to_string();
    if !util::is_valid_name(&name) {
        valid = false;
        errors.push(format!(
            "Invalid element name: {}",
            elem.borrow().as_string()
        ));
    }

    // Inheritance string must resolve to same-category element.
    if elem.borrow().has_inherit_string() {
        let inherit_name = elem.borrow().get_inherit_string();
        let category = elem.borrow().get_category().to_string();
        let resolved = resolve_name_reference(elem, &inherit_name);
        let valid_inherit = resolved
            .as_ref()
            .map(|r| r.borrow().get_category() == category)
            .unwrap_or(false);
        if !valid_inherit {
            valid = false;
            errors.push(format!(
                "Invalid element inheritance: {}",
                elem.borrow().as_string()
            ));
        }
        // Check for cycles in the inheritance chain (C++ Element::validate line ~574).
        // Gather all siblings (same-category elements at root scope) as the search scope.
        if valid_inherit {
            let scope: Vec<ElementPtr> = {
                let root = get_root(elem);
                root.borrow()
                    .children
                    .iter()
                    .filter(|c| c.borrow().get_category() == category)
                    .cloned()
                    .collect()
            };
            if has_inheritance_cycle(elem, &scope) {
                valid = false;
                errors.push(format!(
                    "Cycle in element inheritance chain: {}",
                    elem.borrow().as_string()
                ));
            }
        }
    }

    (valid, errors)
}

/// Validate an element tree recursively: check name validity, inheritance, and child
/// name uniqueness. Returns (valid, all_error_messages).
/// Mirrors C++ Element::validate full traversal.
pub fn validate_element(elem: &ElementPtr) -> (bool, Vec<String>) {
    let mut all_errors = Vec::new();
    let mut all_valid = true;

    // Validate self.
    let (self_valid, self_errors) = validate_element_self(elem);
    if !self_valid {
        all_valid = false;
    }
    all_errors.extend(self_errors);

    // Check for duplicate child names.
    {
        let b = elem.borrow();
        let mut seen = HashSet::new();
        for child in b.get_children() {
            let child_name = child.borrow().get_name().to_string();
            if !seen.insert(child_name.clone()) {
                all_valid = false;
                all_errors.push(format!(
                    "Duplicate child name '{}' in: {}",
                    child_name,
                    b.as_string()
                ));
            }
        }
    }

    // Validate children recursively.
    let children: Vec<ElementPtr> = elem.borrow().get_children().to_vec();
    for child in &children {
        let (child_valid, child_errors) = validate_element(child);
        if !child_valid {
            all_valid = false;
        }
        all_errors.extend(child_errors);
    }

    (all_valid, all_errors)
}

// ---- create_string_resolver ----

/// Create a StringResolver initialised from the element's active context:
/// file prefix, geom prefix, and token substitutions.
/// Optionally accept a geometry name `geom` to apply GeomInfo token substitutions.
/// Mirrors C++ Element::createStringResolver.
pub fn create_string_resolver(elem: &ElementPtr, geom: &str) -> util::StringResolver {
    let mut resolver = util::StringResolver::new();

    // Walk up to collect active file prefix (first ancestor with fileprefix).
    let file_prefix = get_active_file_prefix(elem);
    resolver.set_file_prefix(file_prefix);

    // Walk up to collect active geom prefix.
    let geom_prefix = get_active_geom_prefix(elem);
    resolver.set_geom_prefix(geom_prefix);

    // If a geom string is given, apply geominfo token substitutions.
    if !geom.is_empty() {
        if let Some(doc) = crate::core::document::Document::from_element(elem) {
            for geom_info in doc.get_geom_infos() {
                let info_geom = {
                    let b = geom_info.borrow();
                    b.get_attribute(GEOM_ATTRIBUTE)
                        .map(|s| s.to_string())
                        .unwrap_or_default()
                };
                // Check if geom strings match (reuse geom module helper).
                if !crate::core::geom::geom_strings_match(geom, &info_geom) {
                    continue;
                }
                // Add token substitutions from this geominfo's token children.
                let tokens: Vec<ElementPtr> = geom_info
                    .borrow()
                    .get_children()
                    .iter()
                    .filter(|c| c.borrow().get_category() == category::TOKEN)
                    .cloned()
                    .collect();
                for token in &tokens {
                    let key = format!("<{}>", token.borrow().get_name());
                    let value = token.borrow().get_value_string();
                    resolver.set_filename_substitution(key, value);
                }
            }
        }
    }

    // Add token substitutions from the element itself and its ancestors.
    add_token_substitutions(elem, &mut resolver);

    resolver
}

/// Walk `elem` and its ancestors collecting Token children and registering
/// `<name>` -> value substitutions in the resolver.
/// Mirrors C++ StringResolver::addTokenSubstitutions.
fn add_token_substitutions(elem: &ElementPtr, resolver: &mut util::StringResolver) {
    let mut current: Option<ElementPtr> = Some(elem.clone());
    while let Some(e) = current.take() {
        let tokens: Vec<ElementPtr> = e
            .borrow()
            .get_children()
            .iter()
            .filter(|c| c.borrow().get_category() == category::TOKEN)
            .cloned()
            .collect();
        for token in &tokens {
            let key = format!("<{}>", token.borrow().get_name());
            // Only set if not already present (inner scope wins).
            if !resolver.get_filename_substitutions().contains_key(&key) {
                let value = token.borrow().get_value_string();
                resolver.set_filename_substitution(key, value);
            }
        }
        current = e.borrow().get_parent();
    }
}

/// Return the active file prefix walking up from `elem` through ancestors.
/// Mirrors C++ Element::getActiveFilePrefix.
pub fn get_active_file_prefix(elem: &ElementPtr) -> String {
    let mut current: Option<ElementPtr> = Some(elem.clone());
    while let Some(e) = current.take() {
        if e.borrow().has_file_prefix() {
            return e.borrow().get_file_prefix();
        }
        current = e.borrow().get_parent();
    }
    String::new()
}

/// Return the active geom prefix walking up from `elem` through ancestors.
/// Mirrors C++ Element::getActiveGeomPrefix.
pub fn get_active_geom_prefix(elem: &ElementPtr) -> String {
    let mut current: Option<ElementPtr> = Some(elem.clone());
    while let Some(e) = current.take() {
        if e.borrow().has_geom_prefix() {
            return e.borrow().get_geom_prefix();
        }
        current = e.borrow().get_parent();
    }
    String::new()
}

/// Given two target strings (comma-separated), return true if they share any targets.
/// An empty string matches all targets. C++ targetStringsMatch.
pub fn target_strings_match(target1: &str, target2: &str) -> bool {
    if target1.is_empty() || target2.is_empty() {
        return true;
    }
    let set1: HashSet<&str> = target1.split(',').map(|s| s.trim()).collect();
    target2
        .split(',')
        .map(|s| s.trim())
        .any(|t| set1.contains(t))
}

/// Pretty print element tree (C++ prettyPrint)
pub fn pretty_print(elem: &ElementPtr) -> String {
    fn print_recursive(elem: &ElementPtr, depth: usize, out: &mut String) {
        for _ in 0..depth {
            out.push_str("  ");
        }
        out.push_str(&elem.borrow().as_string());
        out.push('\n');
        let children: Vec<ElementPtr> = elem.borrow().get_children().to_vec();
        for child in &children {
            print_recursive(child, depth + 1, out);
        }
    }
    let mut result = String::new();
    print_recursive(elem, 0, &mut result);
    result
}

// ---- Tree order helpers ----

/// Store an explicit sort key on the element (stored as attribute `__tree_order`).
/// Used by serialisers that need to preserve custom child ordering.
pub fn set_tree_order(elem: &ElementPtr, order: i32) {
    elem.borrow_mut()
        .set_attribute(TREE_ORDER_ATTRIBUTE, order.to_string());
}

/// Retrieve the tree-order key previously set via `set_tree_order`.
/// Returns `None` if the attribute is absent or cannot be parsed.
pub fn get_tree_order(elem: &ElementPtr) -> Option<i32> {
    elem.borrow()
        .get_attribute(TREE_ORDER_ATTRIBUTE)
        .and_then(|s| s.parse::<i32>().ok())
}

// ---- change_child_category ----

/// Change the category of an existing child element, returning the replacement.
/// Preserves child position, name, and all content.  Returns `None` if the
/// named child does not exist.  Mirrors C++ `Element::changeChildCategory`.
pub fn change_child_category(
    parent: &ElementPtr,
    child_name: &str,
    new_category: &str,
) -> Option<ElementPtr> {
    // Locate index so we can restore position after re-insertion.
    let child_index = parent.borrow().get_child_index(child_name)?;

    // Collect the old child's content before removal.
    let old_child = parent.borrow().get_child(child_name)?;
    let old_attrs: Vec<(String, String)> = old_child
        .borrow()
        .iter_attributes()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let old_source_uri = old_child.borrow().source_uri.clone();
    // Snapshot grandchildren to copy later.
    let grandchildren: Vec<ElementPtr> = old_child.borrow().children.clone();

    // Remove the old child.
    parent.borrow_mut().remove_child(child_name);

    // Add a new child with the new category.
    let new_child = add_child_of_category(parent, new_category, child_name).ok()?;

    // Restore position.
    let _ = parent.borrow_mut().set_child_index(child_name, child_index);

    // Copy attributes (skip "name" -- it is structural, not an attribute).
    {
        let mut nb = new_child.borrow_mut();
        for (k, v) in &old_attrs {
            if k != NAME_ATTRIBUTE {
                nb.set_attribute(k.clone(), v.clone());
            }
        }
        nb.source_uri = old_source_uri;
    }

    // Re-attach grandchildren under the new child.
    for gc in grandchildren {
        // Re-parent the grandchild to the new child.
        gc.borrow_mut().parent = Some(new_child.downgrade());
        // Use add_child so child_map stays in sync.
        new_child.borrow_mut().add_child(gc);
    }

    Some(new_child)
}

// ---- resolve_name_reference ----

/// Resolve a name reference relative to the root of `elem`'s tree.
/// Tries the qualified form first (namespace:name), then the bare name.
/// Returns the first matching child of the root, or `None`.
/// Mirrors C++ `Element::resolveNameReference<Element>`.
pub fn resolve_name_reference(elem: &ElementPtr, name: &str) -> Option<ElementPtr> {
    let root = get_root(elem);
    // Build the qualified version of `name` as seen from `elem`.
    let qualified = elem.borrow().get_qualified_name(name);
    // Try qualified form first, then bare name.
    root.borrow()
        .get_child(&qualified)
        .or_else(|| root.borrow().get_child(name))
}

// ---- get_inherits_from (resolved) ----

/// Return the element that `elem` directly inherits from, by resolving
/// the inherit-string attribute against the document root.
/// Returns `None` if there is no inherit string or the target is not found.
/// Mirrors C++ `Element::getInheritsFrom()`.
pub fn get_inherits_from(elem: &ElementPtr) -> Option<ElementPtr> {
    let inherit_name = {
        let b = elem.borrow();
        if !b.has_inherit_string() {
            return None;
        }
        b.get_inherit_string()
    };
    resolve_name_reference(elem, &inherit_name)
}

/// Set the element that `elem` directly inherits from.
/// Passing `None` clears the inherit attribute.
/// Mirrors C++ `Element::setInheritsFrom(ConstElementPtr)`.
pub fn set_inherits_from(elem: &ElementPtr, base: Option<&ElementPtr>) {
    match base {
        Some(b) => {
            let name = b.borrow().get_name().to_string();
            elem.borrow_mut().set_inherit_string(name);
        }
        None => elem.borrow_mut().remove_attribute(INHERIT_ATTRIBUTE),
    }
}

// ---- has_inherited_base ----

/// Return true if `base_name` appears anywhere in `elem`'s inheritance chain.
/// Walks up to `max_depth` steps to protect against cycles.
/// Mirrors C++ `Element::hasInheritedBase(ConstElementPtr)`.
pub fn has_inherited_base(elem: &ElementPtr, base_name: &str) -> bool {
    const MAX_DEPTH: usize = 64;
    let mut current = get_inherits_from(elem);
    for _ in 0..MAX_DEPTH {
        let node = match current {
            None => return false,
            Some(ref e) => e.clone(),
        };
        if node.borrow().get_name() == base_name {
            return true;
        }
        current = get_inherits_from(&node);
    }
    false
}

// ---- Equivalence helpers ----

/// Normalise a float-string to a canonical form for comparison.
/// Parses as f64 then re-formats with the requested precision and format.
fn normalise_float_str(s: &str, format: FloatFormat, precision: i32) -> Option<String> {
    let v: f64 = s.trim().parse().ok()?;
    let prec = precision.max(0) as usize;
    Some(match format {
        FloatFormat::Fixed => format!("{:.prec$}", v, prec = prec),
        FloatFormat::Scientific => format!("{:.prec$e}", v, prec = prec),
        FloatFormat::Default => format!("{:.prec$}", v, prec = prec),
    })
}

/// Normalise a value string that may contain multiple comma-separated floats
/// (e.g. "0.1, 0.2, 0.3" for color3).  Falls back to the raw string if the
/// tokens can't all be parsed as floats.
fn normalise_value_str(s: &str, format: FloatFormat, precision: i32) -> String {
    let parts: Vec<&str> = s.split(',').collect();
    let mut out = Vec::with_capacity(parts.len());
    for part in &parts {
        match normalise_float_str(part.trim(), format, precision) {
            Some(n) => out.push(n),
            None => return s.to_string(), // not purely numeric -- return raw
        }
    }
    out.join(", ")
}

/// Compare a single attribute on two elements for functional equivalence.
/// For value-type attributes (`value`, `uimin`, `uimax`, etc.) this parses
/// and re-formats the floats according to `options` before comparing.
/// Mirrors C++ `Element::isAttributeEquivalent` / `ValueElement::isAttributeEquivalent`.
pub fn is_attribute_equivalent(
    lhs: &ElementPtr,
    rhs: &ElementPtr,
    attr_name: &str,
    options: &ElementEquivalenceOptions,
) -> bool {
    let lhs_val = lhs.borrow().get_attribute_or_empty(attr_name);
    let rhs_val = rhs.borrow().get_attribute_or_empty(attr_name);

    if options.perform_value_comparisons {
        // Attributes that hold typed float values (mirrors C++ ValueElement behaviour).
        const FLOAT_ATTRS: &[&str] = &[
            VALUE_ATTRIBUTE,
            UI_MIN_ATTRIBUTE,
            UI_MAX_ATTRIBUTE,
            UI_SOFT_MIN_ATTRIBUTE,
            UI_SOFT_MAX_ATTRIBUTE,
            UI_STEP_ATTRIBUTE,
        ];
        if FLOAT_ATTRS.contains(&attr_name) {
            let n_lhs =
                normalise_value_str(&lhs_val, options.float_format, options.float_precision);
            let n_rhs =
                normalise_value_str(&rhs_val, options.float_format, options.float_precision);
            return n_lhs == n_rhs;
        }
    }

    lhs_val == rhs_val
}

/// Return true if two element trees are functionally equivalent according to `options`.
/// Comparison covers: category, name, filtered attribute set, recursive children.
/// Comments (category "comment") and newlines are skipped.
/// Mirrors C++ `Element::isEquivalent`.
pub fn is_equivalent(
    lhs: &ElementPtr,
    rhs: &ElementPtr,
    options: &ElementEquivalenceOptions,
) -> bool {
    is_equivalent_impl(lhs, rhs, options)
}

fn is_equivalent_impl(
    lhs: &ElementPtr,
    rhs: &ElementPtr,
    options: &ElementEquivalenceOptions,
) -> bool {
    let lhs_b = lhs.borrow();
    let rhs_b = rhs.borrow();

    // Compare name and category first.
    if lhs_b.get_name() != rhs_b.get_name() {
        return false;
    }
    if lhs_b.get_category() != rhs_b.get_category() {
        return false;
    }

    // Build filtered attribute name lists (sorted, order-independent).
    let filter_attrs = |attrs: Vec<String>| -> Vec<String> {
        let mut v: Vec<String> = attrs
            .into_iter()
            .filter(|a| !options.attribute_exclusion_list.contains(a))
            .collect();
        v.sort();
        v
    };

    let lhs_attrs = filter_attrs(lhs_b.get_attribute_names().cloned().collect());
    let rhs_attrs = filter_attrs(rhs_b.get_attribute_names().cloned().collect());

    if lhs_attrs != rhs_attrs {
        return false;
    }

    // Release borrows before calling is_attribute_equivalent (which borrows again).
    drop(lhs_b);
    drop(rhs_b);

    for attr in &lhs_attrs {
        if !is_attribute_equivalent(lhs, rhs, attr, options) {
            return false;
        }
    }

    // Collect children, skipping comments and newlines (whitespace-only elements).
    let significant_children = |elem: &ElementPtr| -> Vec<ElementPtr> {
        elem.borrow()
            .children
            .iter()
            .filter(|c| {
                let cat = c.borrow().category.clone();
                cat != category::COMMENT && cat != category::NEWLINE
            })
            .cloned()
            .collect()
    };

    let lhs_children = significant_children(lhs);
    let rhs_children = significant_children(rhs);

    if lhs_children.len() != rhs_children.len() {
        return false;
    }

    // C++ Element.cpp:458-478: for compound graphs (Document or NodeGraph without a nodedef)
    // children are matched by name, not by position.
    let lhs_cat = lhs.borrow().get_category().to_string();
    let is_document = lhs_cat == category::DOCUMENT;
    let is_unbound_graph =
        lhs_cat == category::NODE_GRAPH && lhs.borrow().get_attribute("nodedef").is_none();
    let use_name_match = is_document || is_unbound_graph;

    if use_name_match {
        // Build a name->element map from rhs children.
        let mut rhs_map: HashMap<String, ElementPtr> = rhs_children
            .iter()
            .map(|c| (c.borrow().get_name().to_string(), c.clone()))
            .collect();
        for lc in &lhs_children {
            let lc_name = lc.borrow().get_name().to_string();
            match rhs_map.remove(&lc_name) {
                Some(rc) => {
                    if !is_equivalent_impl(lc, &rc, options) {
                        return false;
                    }
                }
                None => return false,
            }
        }
    } else {
        // Ordered comparison for all other element types.
        for (lc, rc) in lhs_children.iter().zip(rhs_children.iter()) {
            if !is_equivalent_impl(lc, rc, options) {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod validate_tests {
    use super::*;
    fn make_doc() -> ElementPtr {
        ElementPtr::new(Element::new(None, category::DOCUMENT, ""))
    }

    // ---- validate_element tests ----

    #[test]
    fn validate_elem_valid_name() {
        let root = make_doc();
        let child = add_child_of_category(&root, "nodedef", "ND_foo").unwrap();
        let (valid, errors) = validate_element(&child);
        assert!(valid, "valid name should pass: {:?}", errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_elem_invalid_name() {
        // Create an element and then directly mutate name to an invalid one.
        let root = make_doc();
        let child = add_child_of_category(&root, "nodedef", "ND_foo").unwrap();
        // Directly set invalid name bypassing set_name validation.
        child.borrow_mut().name = "1invalid".to_string();
        let (valid, errors) = validate_element(&child);
        assert!(!valid, "invalid name should fail");
        assert!(errors.iter().any(|e| e.contains("Invalid element name")));
    }

    #[test]
    fn validate_elem_duplicate_children() {
        let root = make_doc();
        let parent = add_child_of_category(&root, category::NODE_GRAPH, "g1").unwrap();
        // Add first child.
        let c1 = add_child_of_category(&parent, category::NODE, "node1").unwrap();
        // Manually inject a duplicate (bypasses uniqueness check in add_child_of_category).
        let dup = ElementPtr::new(Element::new(
            Some(parent.downgrade()),
            category::NODE,
            "node1",
        ));
        parent.borrow_mut().children.push(dup);
        let _ = c1; // suppress unused warning

        let (valid, errors) = validate_element(&parent);
        assert!(!valid, "duplicate child should fail");
        assert!(errors.iter().any(|e| e.contains("Duplicate child name")));
    }

    // ---- create_string_resolver tests ----

    #[test]
    fn create_resolver_file_prefix_inherited() {
        let root = make_doc();
        root.borrow_mut().set_file_prefix("textures/");
        let child = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();

        let resolver = create_string_resolver(&child, "");
        assert_eq!(
            resolver.get_file_prefix(),
            "textures/",
            "child should inherit parent fileprefix"
        );
    }

    #[test]
    fn create_resolver_local_prefix_wins() {
        let root = make_doc();
        root.borrow_mut().set_file_prefix("textures/");
        let child = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        child.borrow_mut().set_file_prefix("local/");

        let resolver = create_string_resolver(&child, "");
        assert_eq!(
            resolver.get_file_prefix(),
            "local/",
            "local fileprefix should win over parent"
        );
    }

    #[test]
    fn create_resolver_token_substitution() {
        let root = make_doc();
        // Add a token child.
        let token = add_child_of_category(&root, category::TOKEN, "UDIM").unwrap();
        token.borrow_mut().set_value_string("1001");

        let resolver = create_string_resolver(&root, "");
        let subs = resolver.get_filename_substitutions();
        assert!(
            subs.contains_key("<UDIM>"),
            "token UDIM should appear as <UDIM> substitution"
        );
        assert_eq!(subs["<UDIM>"], "1001");
    }

    #[test]
    fn create_resolver_empty_by_default() {
        let root = make_doc();
        let child = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        let resolver = create_string_resolver(&child, "");
        assert!(resolver.get_file_prefix().is_empty());
        assert!(resolver.get_geom_prefix().is_empty());
    }

    // ---- add_newline tests ----

    #[test]
    fn add_newline_basic() {
        let root = make_doc();
        let nl = add_newline(&root).unwrap();
        assert_eq!(nl.borrow().get_category(), category::NEWLINE);
        assert_eq!(root.borrow().get_children().len(), 1);
    }

    #[test]
    fn add_newline_multiple() {
        let root = make_doc();
        let nl1 = add_newline(&root).unwrap();
        let nl2 = add_newline(&root).unwrap();
        // Names must differ.
        assert_ne!(nl1.borrow().get_name(), nl2.borrow().get_name());
        assert_eq!(root.borrow().get_children().len(), 2);
    }

    // ---- get_active_file_prefix / get_active_geom_prefix tests ----

    #[test]
    fn active_file_prefix_walks_up() {
        let root = make_doc();
        root.borrow_mut().set_file_prefix("root_prefix/");
        let child = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        let grandchild = add_child_of_category(&child, category::NODE, "n").unwrap();

        assert_eq!(get_active_file_prefix(&grandchild), "root_prefix/");
    }

    #[test]
    fn active_geom_prefix_from_self() {
        let root = make_doc();
        root.borrow_mut().set_geom_prefix("root_geom/");
        let child = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        child.borrow_mut().set_geom_prefix("child_geom/");

        assert_eq!(get_active_geom_prefix(&child), "child_geom/");
        assert_eq!(get_active_geom_prefix(&root), "root_geom/");
    }
}
