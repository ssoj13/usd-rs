//! MaterialX document model - arena-based immutable document structure.
//!
//! This module provides the core MaterialX document representation using an
//! arena-based approach with Arc<DocumentData> for efficient cloning and
//! element indices for parent/child relationships.

use std::collections::HashMap;
use std::sync::Arc;

/// Error type for MaterialX operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtlxError {
    /// Human-readable error message.
    pub message: String,
}

impl MtlxError {
    /// Creates a new MaterialX error with the given message.
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for MtlxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MaterialX error: {}", self.message)
    }
}

impl std::error::Error for MtlxError {}

/// Internal element storage in the arena
#[derive(Debug, Clone)]
pub struct ElementData {
    /// Element name (from "name" attribute)
    pub name: String,
    /// Element category (XML tag name: "materialx", "nodedef", "node", "input", etc.)
    pub category: String,
    /// All XML attributes (excluding "name")
    pub attributes: HashMap<String, String>,
    /// Indices of child elements
    pub children: Vec<usize>,
    /// Index of parent element (None for root)
    pub parent: Option<usize>,
    /// Source URI where this element was defined
    pub source_uri: String,
}

impl ElementData {
    /// Create a new element with given name and category
    pub fn new(name: String, category: String) -> Self {
        Self {
            name,
            category,
            attributes: HashMap::new(),
            children: Vec::new(),
            parent: None,
            source_uri: String::new(),
        }
    }
}

/// The full MaterialX document (immutable after parsing)
#[derive(Debug, Clone)]
pub struct DocumentData {
    /// All elements in the document (arena storage)
    pub elements: Vec<ElementData>,
    /// Index of the root element
    pub root_idx: usize,
}

impl DocumentData {
    /// Create an empty document with just a root element
    pub fn new() -> Self {
        let mut elements = Vec::new();
        let root = ElementData::new(String::new(), "materialx".to_string());
        elements.push(root);
        Self {
            elements,
            root_idx: 0,
        }
    }

    /// Get element by index
    #[inline]
    pub fn get(&self, idx: usize) -> Option<&ElementData> {
        self.elements.get(idx)
    }

    /// Get mutable element by index (for construction only)
    #[inline]
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut ElementData> {
        self.elements.get_mut(idx)
    }

    /// Add a new element and return its index
    pub fn add_element(&mut self, mut element: ElementData) -> usize {
        let idx = self.elements.len();
        element.source_uri = self.elements[self.root_idx].source_uri.clone();
        self.elements.push(element);
        idx
    }
}

impl Default for DocumentData {
    fn default() -> Self {
        Self::new()
    }
}

/// A MaterialX Document (immutable after construction)
#[derive(Clone, Debug)]
pub struct Document {
    inner: Arc<DocumentData>,
}

impl Document {
    /// Create an empty document
    pub fn create() -> Self {
        Self {
            inner: Arc::new(DocumentData::new()),
        }
    }

    /// Create from DocumentData
    pub(crate) fn from_data(data: DocumentData) -> Self {
        Self {
            inner: Arc::new(data),
        }
    }

    /// Get the root element
    pub fn get_root(&self) -> Element {
        Element {
            doc: Arc::clone(&self.inner),
            idx: self.inner.root_idx,
        }
    }

    /// Get all children of the root element
    pub fn get_children(&self) -> Vec<Element> {
        self.get_root().get_children()
    }

    /// Get all NodeDef elements
    pub fn get_node_defs(&self) -> Vec<NodeDef> {
        self.get_root()
            .get_children_of_type("nodedef")
            .into_iter()
            .map(|e| NodeDef(e))
            .collect()
    }

    /// Get a specific NodeDef by name
    pub fn get_node_def(&self, name: &str) -> Option<NodeDef> {
        self.get_node_defs()
            .into_iter()
            .find(|nd| nd.0.name() == name)
    }

    /// Get all NodeDefs matching a node family (node attribute)
    pub fn get_matching_node_defs(&self, family: &str) -> Vec<NodeDef> {
        self.get_node_defs()
            .into_iter()
            .filter(|nd| nd.get_node_string() == family)
            .collect()
    }

    /// Get all Look elements
    pub fn get_looks(&self) -> Vec<Look> {
        self.get_root()
            .get_children_of_type("look")
            .into_iter()
            .map(|e| Look(e))
            .collect()
    }

    /// Get all TypeDef elements
    pub fn get_type_defs(&self) -> Vec<TypeDef> {
        self.get_root()
            .get_children_of_type("typedef")
            .into_iter()
            .map(|e| TypeDef(e))
            .collect()
    }

    /// Get a TypeDef by type name
    pub fn get_type_def(&self, type_name: &str) -> Option<TypeDef> {
        self.get_type_defs()
            .into_iter()
            .find(|td| td.0.name() == type_name)
    }

    /// Get all NodeGraph elements
    pub fn get_node_graphs(&self) -> Vec<NodeGraph> {
        self.get_root()
            .get_children_of_type("nodegraph")
            .into_iter()
            .map(|e| NodeGraph(e))
            .collect()
    }

    /// Get the active color space for the document
    pub fn get_active_color_space(&self) -> String {
        self.get_root().get_attribute("colorspace").to_string()
    }

    /// Get all referenced source URIs in this document
    pub fn get_referenced_source_uris(&self) -> std::collections::HashSet<String> {
        let mut uris = std::collections::HashSet::new();
        for elem in &self.inner.elements {
            if !elem.source_uri.is_empty() {
                uris.insert(elem.source_uri.clone());
            }
        }
        uris
    }

    /// Import all elements from another document (library merging)
    pub fn import_library(&mut self, other: &Document) {
        // Get mutable reference to inner data
        let inner = Arc::make_mut(&mut self.inner);
        let root_idx = inner.root_idx;
        let base_idx = inner.elements.len();
        let other_root = other.inner.root_idx;

        // Build a mapping from old indices to new indices, skipping root.
        // Non-root elements are compacted: old_idx -> base_idx + count.
        let mut old_to_new: Vec<Option<usize>> = vec![None; other.inner.elements.len()];
        let mut count = 0usize;
        for idx in 0..other.inner.elements.len() {
            if idx == other_root {
                continue;
            }
            old_to_new[idx] = Some(base_idx + count);
            count += 1;
        }

        // Track which elements need to be added as root children
        let mut new_root_children = Vec::new();

        // Copy all non-root elements from other document
        for (idx, elem) in other.inner.elements.iter().enumerate() {
            if idx == other_root {
                continue; // Skip other's root
            }

            let mut new_elem = elem.clone();
            let new_idx = old_to_new[idx].unwrap();

            // Update parent references
            if let Some(parent_idx) = new_elem.parent {
                if parent_idx == other_root {
                    // Parent was other's root, now it's our root
                    new_elem.parent = Some(root_idx);
                    new_root_children.push(new_idx);
                } else if let Some(mapped) = old_to_new[parent_idx] {
                    new_elem.parent = Some(mapped);
                } else {
                    // Parent was root (shouldn't happen); fallback to our root
                    new_elem.parent = Some(root_idx);
                    new_root_children.push(new_idx);
                }
            }

            // Update child indices
            for child in &mut new_elem.children {
                if let Some(mapped) = old_to_new[*child] {
                    *child = mapped;
                }
                // Children pointing to root are dropped (shouldn't happen)
            }

            inner.elements.push(new_elem);
        }

        // Add new elements to root's children list
        inner.elements[root_idx].children.extend(new_root_children);
    }

    /// Get direct access to inner data (for advanced usage)
    pub(crate) fn inner(&self) -> &Arc<DocumentData> {
        &self.inner
    }
}

/// Handle to an element within a document
#[derive(Clone, Debug)]
pub struct Element {
    doc: Arc<DocumentData>,
    idx: usize,
}

impl Element {
    /// Create element from document and index
    #[allow(dead_code)]
    pub(crate) fn new(doc: Arc<DocumentData>, idx: usize) -> Self {
        Self { doc, idx }
    }

    /// Get element data
    #[inline]
    fn data(&self) -> &ElementData {
        &self.doc.elements[self.idx]
    }

    /// Get element name
    pub fn name(&self) -> &str {
        &self.data().name
    }

    /// Get element category (XML tag name)
    pub fn category(&self) -> &str {
        &self.data().category
    }

    /// Get attribute value (returns empty string if not found, like C++)
    pub fn get_attribute(&self, name: &str) -> &str {
        self.data()
            .attributes
            .get(name)
            .map(|s| s.as_str())
            .unwrap_or(EMPTY_STRING)
    }

    /// Check if attribute exists
    pub fn has_attribute(&self, name: &str) -> bool {
        self.data().attributes.contains_key(name)
    }

    /// Get all child elements
    pub fn get_children(&self) -> Vec<Element> {
        self.data()
            .children
            .iter()
            .map(|&idx| Element {
                doc: Arc::clone(&self.doc),
                idx,
            })
            .collect()
    }

    /// Get children of a specific category
    pub fn get_children_of_type(&self, category: &str) -> Vec<Element> {
        self.get_children()
            .into_iter()
            .filter(|e| e.category() == category)
            .collect()
    }

    /// Get parent element
    pub fn get_parent(&self) -> Option<Element> {
        self.data().parent.map(|idx| Element {
            doc: Arc::clone(&self.doc),
            idx,
        })
    }

    /// Get root element
    pub fn get_root(&self) -> Element {
        Element {
            doc: Arc::clone(&self.doc),
            idx: self.doc.root_idx,
        }
    }

    /// Get the document this element belongs to
    pub fn get_document(&self) -> Document {
        Document {
            inner: Arc::clone(&self.doc),
        }
    }

    /// Get source URI
    pub fn get_source_uri(&self) -> &str {
        &self.data().source_uri
    }

    /// Get child by name
    pub fn get_child(&self, name: &str) -> Option<Element> {
        self.get_children().into_iter().find(|e| e.name() == name)
    }

    /// Check if this element is of a specific category
    pub fn is_a(&self, category: &str) -> bool {
        self.category() == category
    }

    /// Get full name path (e.g., "material1/shader1/input1")
    pub fn get_name_path(&self) -> String {
        let mut path_parts = Vec::new();
        let mut current = Some(self.clone());

        while let Some(elem) = current {
            if elem.idx == self.doc.root_idx {
                break; // Don't include root
            }
            if !elem.name().is_empty() {
                path_parts.push(elem.name().to_string());
            }
            current = elem.get_parent();
        }

        path_parts.reverse();
        path_parts.join("/")
    }

    /// Get debug string representation
    pub fn as_string(&self) -> String {
        format!(
            "<{} name=\"{}\" at {}>",
            self.category(),
            self.name(),
            self.get_name_path()
        )
    }

    /// Get element index (for internal use)
    #[allow(dead_code)]
    pub(crate) fn index(&self) -> usize {
        self.idx
    }
}

/// NodeDef element (category = "nodedef")
#[derive(Clone, Debug)]
pub struct NodeDef(pub Element);

impl NodeDef {
    /// Create from Element (checks category)
    pub fn try_from(elem: Element) -> Option<Self> {
        if elem.is_a("nodedef") {
            Some(Self(elem))
        } else {
            None
        }
    }

    /// Get the node string (node attribute)
    pub fn get_node_string(&self) -> &str {
        self.0.get_attribute("node")
    }

    /// Get version string
    pub fn get_version_string(&self) -> &str {
        self.0.get_attribute("version")
    }

    /// Check if this is the default version
    pub fn get_default_version(&self) -> bool {
        self.0.get_attribute("isdefaultversion") == "true"
    }

    /// Get target string
    pub fn get_target(&self) -> &str {
        self.0.get_attribute("target")
    }

    /// Check if has inherit attribute
    pub fn has_inherit_string(&self) -> bool {
        self.0.has_attribute("inherit")
    }

    /// Get inherit string
    pub fn get_inherit_string(&self) -> &str {
        self.0.get_attribute("inherit")
    }

    /// Get type string
    pub fn get_type(&self) -> &str {
        self.0.get_attribute("type")
    }

    /// Get all input elements
    pub fn get_inputs(&self) -> Vec<Input> {
        self.0
            .get_children_of_type("input")
            .into_iter()
            .map(|e| Input(e))
            .collect()
    }

    /// Get all output elements
    pub fn get_outputs(&self) -> Vec<Output> {
        self.0
            .get_children_of_type("output")
            .into_iter()
            .map(|e| Output(e))
            .collect()
    }

    /// Get active inputs (including inherited), with cycle detection.
    pub fn get_active_inputs(&self) -> Vec<Input> {
        let mut visited = std::collections::HashSet::new();
        self.get_active_inputs_impl(&mut visited)
    }

    /// Internal recursive helper with visited set for cycle detection.
    fn get_active_inputs_impl(
        &self,
        visited: &mut std::collections::HashSet<String>,
    ) -> Vec<Input> {
        let mut inputs = self.get_inputs();

        if self.has_inherit_string() {
            let inherit_name = self.get_inherit_string().to_string();
            // Cycle detection: skip if already visited
            if visited.insert(inherit_name.clone()) {
                if let Some(base_nodedef) = self.0.get_document().get_node_def(&inherit_name) {
                    let base_inputs = base_nodedef.get_active_inputs_impl(visited);
                    for base_input in base_inputs {
                        if !inputs.iter().any(|i| i.0.name() == base_input.0.name()) {
                            inputs.push(base_input);
                        }
                    }
                }
            }
        }

        inputs
    }

    /// Get active outputs (including inherited), with cycle detection.
    pub fn get_active_outputs(&self) -> Vec<Output> {
        let mut visited = std::collections::HashSet::new();
        self.get_active_outputs_impl(&mut visited)
    }

    /// Internal recursive helper with visited set for cycle detection.
    fn get_active_outputs_impl(
        &self,
        visited: &mut std::collections::HashSet<String>,
    ) -> Vec<Output> {
        let mut outputs = self.get_outputs();

        if self.has_inherit_string() {
            let inherit_name = self.get_inherit_string().to_string();
            if visited.insert(inherit_name.clone()) {
                if let Some(base_nodedef) = self.0.get_document().get_node_def(&inherit_name) {
                    let base_outputs = base_nodedef.get_active_outputs_impl(visited);
                    for base_output in base_outputs {
                        if !outputs.iter().any(|o| o.0.name() == base_output.0.name()) {
                            outputs.push(base_output);
                        }
                    }
                }
            }
        }

        outputs
    }

    /// Find matching implementation (nodegraph or implementation element)
    pub fn get_implementation(&self) -> Option<Element> {
        let doc = self.0.get_document();
        let root = doc.get_root();

        // Search for matching nodegraph or implementation
        for child in root.get_children() {
            if child.category() == "nodegraph" || child.category() == "implementation" {
                if child.get_attribute("nodedef") == self.0.name() {
                    return Some(child);
                }
            }
        }

        None
    }
}

/// Node element (category = "node")
#[derive(Clone, Debug)]
pub struct Node(pub Element);

impl Node {
    /// Create from Element
    pub fn try_from(elem: Element) -> Option<Self> {
        if elem.is_a("node") {
            Some(Self(elem))
        } else {
            None
        }
    }

    /// Get the node category name (from "node" attribute or element name)
    pub fn get_category_name(&self) -> &str {
        let node_attr = self.0.get_attribute("node");
        if !node_attr.is_empty() {
            node_attr
        } else {
            self.0.category()
        }
    }

    /// Get type string
    pub fn get_type(&self) -> &str {
        self.0.get_attribute("type")
    }

    /// Get target string
    pub fn get_target(&self) -> &str {
        self.0.get_attribute("target")
    }

    /// Get nodedef string attribute
    pub fn get_node_def_string(&self) -> &str {
        self.0.get_attribute("nodedef")
    }

    /// Check if has nodedef attribute
    pub fn has_node_def_string(&self) -> bool {
        self.0.has_attribute("nodedef")
    }

    /// Find matching NodeDef
    pub fn get_node_def(&self) -> Option<NodeDef> {
        let doc = self.0.get_document();

        // First try explicit nodedef attribute
        if self.has_node_def_string() {
            return doc.get_node_def(self.get_node_def_string());
        }

        // Otherwise search by node category
        let matching = doc.get_matching_node_defs(self.get_category_name());
        matching.into_iter().next()
    }

    /// Get all inputs
    pub fn get_inputs(&self) -> Vec<Input> {
        self.0
            .get_children_of_type("input")
            .into_iter()
            .map(|e| Input(e))
            .collect()
    }

    /// Get all outputs
    pub fn get_outputs(&self) -> Vec<Output> {
        self.0
            .get_children_of_type("output")
            .into_iter()
            .map(|e| Output(e))
            .collect()
    }
}

/// Input element (category = "input")
#[derive(Clone, Debug)]
pub struct Input(pub Element);

impl Input {
    /// Create from Element
    pub fn try_from(elem: Element) -> Option<Self> {
        if elem.is_a("input") {
            Some(Self(elem))
        } else {
            None
        }
    }

    /// Get type string
    pub fn get_type(&self) -> &str {
        self.0.get_attribute("type")
    }

    /// Get value string
    pub fn get_value_string(&self) -> &str {
        self.0.get_attribute("value")
    }

    /// Get node name (for node connections)
    pub fn get_node_name(&self) -> &str {
        self.0.get_attribute("nodename")
    }

    /// Get output attribute (for connections)
    pub fn get_output(&self) -> &str {
        self.0.get_attribute("output")
    }

    /// Get interface name
    pub fn get_interface_name(&self) -> &str {
        self.0.get_attribute("interfacename")
    }

    /// Get color space
    pub fn get_color_space(&self) -> &str {
        self.0.get_attribute("colorspace")
    }

    /// Get active color space (walk up parents)
    pub fn get_active_color_space(&self) -> String {
        let cs = self.get_color_space();
        if !cs.is_empty() {
            return cs.to_string();
        }

        // Walk up to find colorspace
        let mut current = self.0.get_parent();
        while let Some(elem) = current {
            let cs = elem.get_attribute("colorspace");
            if !cs.is_empty() {
                return cs.to_string();
            }
            current = elem.get_parent();
        }

        String::new()
    }
}

/// Output element (category = "output")
#[derive(Clone, Debug)]
pub struct Output(pub Element);

impl Output {
    /// Create from Element
    pub fn try_from(elem: Element) -> Option<Self> {
        if elem.is_a("output") {
            Some(Self(elem))
        } else {
            None
        }
    }

    /// Get type string
    pub fn get_type(&self) -> &str {
        self.0.get_attribute("type")
    }

    /// Get node name
    pub fn get_node_name(&self) -> &str {
        self.0.get_attribute("nodename")
    }
}

/// NodeGraph element (category = "nodegraph")
#[derive(Clone, Debug)]
pub struct NodeGraph(pub Element);

impl NodeGraph {
    /// Create from Element
    pub fn try_from(elem: Element) -> Option<Self> {
        if elem.is_a("nodegraph") {
            Some(Self(elem))
        } else {
            None
        }
    }

    /// Get all nodes of a specific category
    pub fn get_nodes(&self, category: &str) -> Vec<Node> {
        self.0
            .get_children_of_type("node")
            .into_iter()
            .filter(|e| e.get_attribute("node") == category || category.is_empty())
            .map(|e| Node(e))
            .collect()
    }

    /// Get all inputs
    pub fn get_inputs(&self) -> Vec<Input> {
        self.0
            .get_children_of_type("input")
            .into_iter()
            .map(|e| Input(e))
            .collect()
    }

    /// Get all outputs
    pub fn get_outputs(&self) -> Vec<Output> {
        self.0
            .get_children_of_type("output")
            .into_iter()
            .map(|e| Output(e))
            .collect()
    }
}

/// Look element (category = "look")
#[derive(Clone, Debug)]
pub struct Look(pub Element);

impl Look {
    /// Get all material assignments
    pub fn get_material_assigns(&self) -> Vec<MaterialAssign> {
        self.0
            .get_children_of_type("materialassign")
            .into_iter()
            .map(|e| MaterialAssign(e))
            .collect()
    }

    /// Get look name
    pub fn get_name(&self) -> &str {
        self.0.name()
    }
}

/// MaterialAssign element (category = "materialassign")
#[derive(Clone, Debug)]
pub struct MaterialAssign(pub Element);

impl MaterialAssign {
    /// Get material reference
    pub fn get_material(&self) -> &str {
        self.0.get_attribute("material")
    }
}

/// Collection element (category = "collection")
#[derive(Clone, Debug)]
pub struct Collection(pub Element);

impl Collection {
    /// Create from Element
    pub fn try_from(elem: Element) -> Option<Self> {
        if elem.is_a("collection") {
            Some(Self(elem))
        } else {
            None
        }
    }
}

/// Material element (category = "material")
#[derive(Clone, Debug)]
pub struct Material(pub Element);

impl Material {
    /// Create from Element
    pub fn try_from(elem: Element) -> Option<Self> {
        if elem.is_a("material") {
            Some(Self(elem))
        } else {
            None
        }
    }
}

/// GeomInfo element (category = "geominfo")
#[derive(Clone, Debug)]
pub struct GeomInfo(pub Element);

/// VariantSet element (category = "variantset")
#[derive(Clone, Debug)]
pub struct VariantSet(pub Element);

/// Variant element (category = "variant")
#[derive(Clone, Debug)]
pub struct Variant(pub Element);

/// TypeDef element (category = "typedef")
#[derive(Clone, Debug)]
pub struct TypeDef(pub Element);

impl TypeDef {
    /// Get the type name
    pub fn get_name(&self) -> &str {
        self.0.name()
    }

    /// Get the semantic
    pub fn get_semantic(&self) -> &str {
        self.0.get_attribute("semantic")
    }

    /// Get the context
    pub fn get_context(&self) -> &str {
        self.0.get_attribute("context")
    }
}

// MaterialX constants (from C++ MaterialX Core)

/// Surface shader type string
pub const SURFACE_SHADER_TYPE_STRING: &str = "surfaceshader";

/// Displacement shader type string
pub const DISPLACEMENT_SHADER_TYPE_STRING: &str = "displacementshader";

/// Volume shader type string
pub const VOLUME_SHADER_TYPE_STRING: &str = "volumeshader";

/// Light shader type string
pub const LIGHT_SHADER_TYPE_STRING: &str = "lightshader";

/// Shader semantic
pub const SHADER_SEMANTIC: &str = "shader";

/// Preferred separator for array values
pub const ARRAY_PREFERRED_SEPARATOR: &str = ", ";

/// Empty string constant (like C++ MaterialX)
pub const EMPTY_STRING: &str = "";

// Trait for typed elements

/// Trait for elements that have a type attribute.
pub trait TypedElement {
    /// Returns the value of the `type` attribute.
    fn get_type(&self) -> &str;
}

impl TypedElement for Input {
    fn get_type(&self) -> &str {
        Input::get_type(self)
    }
}

impl TypedElement for Output {
    fn get_type(&self) -> &str {
        Output::get_type(self)
    }
}

impl TypedElement for NodeDef {
    fn get_type(&self) -> &str {
        NodeDef::get_type(self)
    }
}

impl TypedElement for Node {
    fn get_type(&self) -> &str {
        Node::get_type(self)
    }
}

/// Trait for elements that have values and color spaces.
pub trait ValueElement {
    /// Returns the raw value string from the `value` attribute.
    fn get_value_string(&self) -> &str;
    /// Returns the `colorspace` attribute value.
    fn get_color_space(&self) -> &str;
    /// Returns the active color space (inherited or explicitly set).
    fn get_active_color_space(&self) -> String;
}

impl ValueElement for Input {
    fn get_value_string(&self) -> &str {
        Input::get_value_string(self)
    }

    fn get_color_space(&self) -> &str {
        Input::get_color_space(self)
    }

    fn get_active_color_space(&self) -> String {
        Input::get_active_color_space(self)
    }
}

/// Trait for elements with interface (inputs/outputs).
pub trait InterfaceElement {
    /// Returns all input elements.
    fn get_inputs(&self) -> Vec<Input>;
    /// Returns all output elements.
    fn get_outputs(&self) -> Vec<Output>;

    /// Check if this interface exactly matches another.
    fn has_exact_input_match(&self, other: &dyn InterfaceElement) -> bool {
        let self_inputs = self.get_inputs();
        let other_inputs = other.get_inputs();

        if self_inputs.len() != other_inputs.len() {
            return false;
        }

        for (a, b) in self_inputs.iter().zip(other_inputs.iter()) {
            if a.0.name() != b.0.name() || a.get_type() != b.get_type() {
                return false;
            }
        }

        true
    }
}

impl InterfaceElement for NodeDef {
    fn get_inputs(&self) -> Vec<Input> {
        NodeDef::get_inputs(self)
    }

    fn get_outputs(&self) -> Vec<Output> {
        NodeDef::get_outputs(self)
    }
}

impl InterfaceElement for NodeGraph {
    fn get_inputs(&self) -> Vec<Input> {
        NodeGraph::get_inputs(self)
    }

    fn get_outputs(&self) -> Vec<Output> {
        NodeGraph::get_outputs(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document() {
        let doc = Document::create();
        let root = doc.get_root();
        assert_eq!(root.category(), "materialx");
        assert_eq!(root.get_children().len(), 0);
    }

    #[test]
    fn test_element_navigation() {
        let mut data = DocumentData::new();
        let mut child = ElementData::new("test_node".to_string(), "node".to_string());
        child
            .attributes
            .insert("type".to_string(), "float".to_string());
        child.parent = Some(0);

        let child_idx = data.add_element(child);
        data.elements[0].children.push(child_idx);

        let doc = Document::from_data(data);
        let root = doc.get_root();
        let children = root.get_children();

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name(), "test_node");
        assert_eq!(children[0].category(), "node");
        assert_eq!(children[0].get_attribute("type"), "float");
        assert_eq!(children[0].get_parent().unwrap().category(), "materialx");
    }

    #[test]
    fn test_nodedef_wrapper() {
        let mut data = DocumentData::new();
        let mut nodedef = ElementData::new("ND_test".to_string(), "nodedef".to_string());
        nodedef
            .attributes
            .insert("node".to_string(), "test".to_string());
        nodedef
            .attributes
            .insert("version".to_string(), "1.0".to_string());
        nodedef
            .attributes
            .insert("type".to_string(), "color3".to_string());
        nodedef.parent = Some(0);

        let nd_idx = data.add_element(nodedef);
        data.elements[0].children.push(nd_idx);

        let doc = Document::from_data(data);
        let nodedefs = doc.get_node_defs();

        assert_eq!(nodedefs.len(), 1);
        assert_eq!(nodedefs[0].get_node_string(), "test");
        assert_eq!(nodedefs[0].get_version_string(), "1.0");
        assert_eq!(nodedefs[0].get_type(), "color3");
    }

    #[test]
    fn test_get_name_path() {
        let mut data = DocumentData::new();

        let mut parent = ElementData::new("material1".to_string(), "material".to_string());
        parent.parent = Some(0);
        let parent_idx = data.add_element(parent);
        data.elements[0].children.push(parent_idx);

        let mut child = ElementData::new("shader1".to_string(), "node".to_string());
        child.parent = Some(parent_idx);
        let child_idx = data.add_element(child);
        data.elements[parent_idx].children.push(child_idx);

        let doc = Document::from_data(data);
        let elem = Element::new(doc.inner().clone(), child_idx);

        assert_eq!(elem.get_name_path(), "material1/shader1");
    }
}
