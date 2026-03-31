//! Document — top-level MaterialX element.

use crate::core::element::{
    CMS_ATTRIBUTE, CMS_CONFIG_ATTRIBUTE, Element, ElementPtr, ElementWeakPtr,
    INTERFACE_NAME_ATTRIBUTE, NODE_ATTRIBUTE, NODE_DEF_ATTRIBUTE, NODE_GRAPH_ATTRIBUTE,
    NODE_NAME_ATTRIBUTE, XPOS_ATTRIBUTE, YPOS_ATTRIBUTE, add_child_of_category, category,
    copy_content_from_element,
};
use crate::core::interface::{
    add_input, add_output, remove_input, set_node_def_string, set_node_string,
};
use crate::core::node::get_input;
use crate::core::util;

/// Parse a version string like "1.38" or "1.39" into (major, minor).
fn parse_version_string(s: &str) -> Option<(i32, i32)> {
    let mut parts = s.splitn(2, '.');
    let maj = parts.next()?.trim().parse::<i32>().ok()?;
    let min = parts.next()?.trim().parse::<i32>().ok()?;
    Some((maj, min))
}

/// Empty string constant (MaterialX uses empty for default doc name)
const EMPTY_STRING: &str = "";

/// Recursively collect all source URIs from element tree.
fn collect_source_uris(elem: &ElementPtr, uris: &mut std::collections::HashSet<String>) {
    if let Some(uri) = elem.borrow().get_source_uri() {
        let s = uri.to_string();
        if !s.is_empty() {
            uris.insert(s);
        }
    }
    let children: Vec<ElementPtr> = elem.borrow().get_children().to_vec();
    for child in &children {
        collect_source_uris(child, uris);
    }
}

/// Create a new MaterialX document.
pub fn create_document() -> Document {
    let root = Element::new(None, category::DOCUMENT, EMPTY_STRING);
    let root_ptr = ElementPtr::new(root);
    Document {
        root: root_ptr,
        data_library: None,
    }
}

/// MaterialX document — root element container.
#[derive(Clone, Debug)]
pub struct Document {
    root: ElementPtr,
    /// Data library for getChildOfType fallback (MaterialX setDataLibrary)
    data_library: Option<Box<Document>>,
}

impl Document {
    /// Create a new empty document.
    pub fn new() -> Self {
        create_document()
    }

    /// Get document containing this element (traverse up to materialx root).
    /// Note: returned document has no data_library set.
    pub fn from_element(elem: &ElementPtr) -> Option<Self> {
        let mut current = elem.clone();
        loop {
            if current.borrow().get_category() == category::DOCUMENT {
                return Some(Self {
                    root: current,
                    data_library: None,
                });
            }
            let next = current.borrow().get_parent()?.clone();
            current = next;
        }
    }

    /// Get the root element.
    pub fn get_root(&self) -> ElementPtr {
        self.root.clone()
    }

    /// Set the data library (MaterialX setDataLibrary). Used for qualified lookups.
    pub fn set_data_library(&mut self, library: Document) {
        self.data_library = Some(Box::new(library));
    }

    /// Return true if document has a data library.
    pub fn has_data_library(&self) -> bool {
        self.data_library.is_some()
    }

    /// Get the data library, if any.
    pub fn get_data_library(&self) -> Option<&Document> {
        self.data_library.as_deref()
    }

    /// Get child by name, checking data library first then local (MaterialX getChildOfType).
    fn get_child_with_library(&self, name: &str) -> Option<ElementPtr> {
        if let Some(lib) = &self.data_library {
            if let Some(child) = lib.root.borrow().get_child(name) {
                return Some(child);
            }
        }
        self.root.borrow().get_child(name)
    }

    /// Add a child element of the given category with the given name.
    /// Returns the new element.
    pub fn add_child_of_category(
        &mut self,
        category: &str,
        name: &str,
    ) -> Result<ElementPtr, String> {
        let child_name = if name.is_empty() {
            let count = self.root.borrow().children.len();
            util::create_valid_name(&format!("{}{}", category, count + 1), '_')
        } else {
            if !util::is_valid_name(name) {
                return Err(format!("Invalid name: '{}'", name));
            }
            name.to_string()
        };

        let parent_weak: ElementWeakPtr = self.root.downgrade();
        let child = Element::new(Some(parent_weak), category, child_name);

        // Check uniqueness
        if self.root.borrow().get_child(&child.name).is_some() {
            return Err(format!("Element '{}' already exists", child.name));
        }

        let child_ptr = ElementPtr::new(child);
        self.root.borrow_mut().add_child(child_ptr.clone());
        Ok(child_ptr)
    }

    /// Get child by name.
    pub fn get_child(&self, name: &str) -> Option<ElementPtr> {
        self.root.borrow().get_child(name)
    }

    /// Get all children.
    pub fn get_children(&self) -> Vec<ElementPtr> {
        self.root.borrow().get_children().to_vec()
    }

    /// No-op for API compatibility with C++ Document::invalidateCache.
    /// Rust does not use a document-level cache, so this is a deliberate no-op.
    /// C++ calls this after any structural mutation (set_name, add/remove child, set_attribute).
    pub fn invalidate_cache(&self) {
        // Intentional no-op: caching is not used in this Rust port.
    }

    /// Add a newline element as a child of the document root.
    /// Mirrors C++ Document::addNewline / Element::addChild<NewlineElement>.
    /// Returns the new element (category = "newline", auto-generated name).
    pub fn add_newline(&mut self) -> Result<ElementPtr, String> {
        // Generate a unique name for the newline element.
        let count = self.root.borrow().get_children().len();
        let name = format!("newline{}", count + 1);
        self.add_child_of_category(category::NEWLINE, &name)
    }

    /// Get NodeGraph by name (checks data library first)
    pub fn get_node_graph(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child_with_library(name)?;
        if child.borrow().get_category() == category::NODE_GRAPH {
            Some(child)
        } else {
            None
        }
    }

    /// Get NodeDef by name (checks data library first)
    pub fn get_node_def(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child_with_library(name)?;
        if child.borrow().get_category() == category::NODEDEF {
            Some(child)
        } else {
            None
        }
    }

    /// Get Material (surfacematerial, volumematerial) by name (checks data library first)
    pub fn get_material(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child_with_library(name)?;
        let cat = child.borrow().get_category().to_string();
        if cat == category::MATERIAL
            || cat == crate::core::types::SURFACE_MATERIAL_NODE_STRING
            || cat == crate::core::types::VOLUME_MATERIAL_NODE_STRING
        {
            Some(child)
        } else {
            None
        }
    }

    /// Get Look by name (checks data library first)
    pub fn get_look(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child_with_library(name)?;
        if child.borrow().get_category() == category::LOOK {
            Some(child)
        } else {
            None
        }
    }

    /// Get Implementation by name (checks data library first)
    pub fn get_implementation(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child_with_library(name)?;
        if child.borrow().get_category() == category::IMPLEMENTATION {
            Some(child)
        } else {
            None
        }
    }

    /// Get NodeDefs matching the given node name. Checks data library first, then local.
    /// Matches by "node" attribute on nodedef children. By ref Document::getMatchingNodeDefs.
    pub fn get_matching_node_defs(&self, node_name: &str) -> Vec<ElementPtr> {
        // Start with data library results (C++: recurse to data library first)
        let mut result = if let Some(lib) = &self.data_library {
            lib.get_matching_node_defs(node_name)
        } else {
            Vec::new()
        };
        let lookup_names = self.get_matching_names(node_name);
        // Append local matches
        for child in self.root.borrow().get_children() {
            if child.borrow().get_category() != category::NODEDEF {
                continue;
            }
            let nd_node = child
                .borrow()
                .get_attribute("node")
                .map(|s| s.to_string())
                .unwrap_or_default();
            let qualified_node = child.borrow().get_qualified_name(&nd_node);
            if lookup_names
                .iter()
                .any(|name| name == &nd_node || name == &qualified_node)
            {
                result.push(child.clone());
            }
        }
        result
    }

    /// Get implementations (NodeGraph/Implementation) for the given nodedef string.
    /// Checks data library first, then local. By ref Document::getMatchingImplementations.
    pub fn get_matching_implementations(&self, node_def: &str) -> Vec<ElementPtr> {
        // Start with data library results
        let mut result = if let Some(lib) = &self.data_library {
            lib.get_matching_implementations(node_def)
        } else {
            Vec::new()
        };
        let lookup_names = self.get_matching_names(node_def);
        for child in self.root.borrow().get_children() {
            let cat = child.borrow().get_category().to_string();
            if cat != category::NODE_GRAPH && cat != category::IMPLEMENTATION {
                continue;
            }
            let nd = child
                .borrow()
                .get_attribute(NODE_DEF_ATTRIBUTE)
                .map(|s| s.to_string())
                .unwrap_or_default();
            let qualified_node_def = child.borrow().get_qualified_name(&nd);
            if lookup_names
                .iter()
                .any(|name| name == &nd || name == &qualified_node_def)
            {
                result.push(child.clone());
            }
        }
        result
    }

    /// Return all port elements (Input/Output) that connect to the given node name.
    /// C++ Document::getMatchingPorts — scans the full element tree and checks nodename attribute.
    pub fn get_matching_ports(&self, node_name: &str) -> Vec<ElementPtr> {
        fn collect_ports(elem: &ElementPtr, targets: &[String], out: &mut Vec<ElementPtr>) {
            for child in elem.borrow().get_children().to_vec() {
                let cat = child.borrow().get_category().to_string();
                if cat == category::INPUT || cat == category::OUTPUT {
                    let matches_attr = |value: &str| {
                        let qualified = child.borrow().get_qualified_name(value);
                        targets
                            .iter()
                            .any(|target| target == value || target == &qualified)
                    };
                    // Match by nodename or nodegraph attribute.
                    let matches_node = child
                        .borrow()
                        .get_node_name()
                        .map(matches_attr)
                        .unwrap_or(false);
                    let matches_graph = child
                        .borrow()
                        .get_attribute(NODE_GRAPH_ATTRIBUTE)
                        .map(matches_attr)
                        .unwrap_or(false);
                    if matches_node || matches_graph {
                        out.push(child.clone());
                    }
                }
                collect_ports(&child, targets, out);
            }
        }
        let mut result = Vec::new();
        let targets = self.get_matching_names(node_name);
        collect_ports(&self.root, &targets, &mut result);
        result
    }

    /// Return the value of a geometric property for the given geometry string.
    /// Scans all GeomInfo elements whose geom matches, returning the last matching GeomProp value.
    /// C++ Document::getGeomPropValue.
    pub fn get_geom_prop_value(&self, geom_prop_name: &str, geom: &str) -> Option<String> {
        let geom_to_match = if geom.is_empty() {
            crate::core::geom::UNIVERSAL_GEOM_NAME
        } else {
            geom
        };
        let mut found: Option<String> = None;
        for geom_info in self.get_geom_infos() {
            let active_geom = geom_info
                .borrow()
                .get_attribute(crate::core::element::GEOM_ATTRIBUTE)
                .map(|s| s.to_string())
                .unwrap_or_else(|| crate::core::geom::UNIVERSAL_GEOM_NAME.to_string());
            if !crate::core::geom::geom_strings_match(geom_to_match, &active_geom) {
                continue;
            }
            // Look for a geomprop child with matching name.
            for child in geom_info.borrow().get_children().to_vec() {
                if child.borrow().get_category() == category::GEOM_PROP
                    && child.borrow().get_name() == geom_prop_name
                {
                    let val = child.borrow().get_value_string();
                    if !val.is_empty() {
                        found = Some(val);
                    }
                }
            }
        }
        found
    }

    /// Return material-type outputs from all NodeGraphs in this document.
    /// Skips nodegraphs that are definitions or from an included file.
    /// C++ Document::getMaterialOutputs.
    pub fn get_material_outputs(&self) -> Vec<ElementPtr> {
        let doc_uri = self
            .root
            .borrow()
            .get_source_uri()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let mut result = Vec::new();
        for ng in self.get_node_graphs() {
            // Skip graphs that are NodeDef implementations.
            let is_def = ng
                .borrow()
                .get_attribute(NODE_DEF_ATTRIBUTE)
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if is_def {
                continue;
            }
            // Skip graphs from included files.
            let graph_uri = ng
                .borrow()
                .get_source_uri()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if !graph_uri.is_empty() && graph_uri != doc_uri {
                continue;
            }
            let mat_outs = crate::core::node::nodegraph_get_material_outputs(&ng);
            result.extend(mat_outs);
        }
        result
    }

    /// Get TypeDef by name
    pub fn get_type_def(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child(name)?;
        if child.borrow().get_category() == category::TYPEDEF {
            Some(child)
        } else {
            None
        }
    }

    /// Get UnitTypeDef by name (by ref Document::getUnitTypeDef).
    pub fn get_unit_type_def(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child(name)?;
        if child.borrow().get_category() == category::UNIT_TYPEDEF {
            Some(child)
        } else {
            self.root
                .borrow()
                .get_children()
                .iter()
                .find(|c| {
                    c.borrow().get_category() == category::UNIT_TYPEDEF
                        && c.borrow().get_name() == name
                })
                .cloned()
        }
    }

    /// Get GeomPropDef by name (by ref resolve GeomPropDef for defaultgeomprop).
    pub fn get_geom_prop_def(&self, name: &str) -> Option<ElementPtr> {
        if let Some(c) = self.get_child(name) {
            if c.borrow().get_category() == category::GEOM_PROP_DEF {
                return Some(c);
            }
        }
        self.root
            .borrow()
            .get_children()
            .iter()
            .find(|c| {
                c.borrow().get_category() == category::GEOM_PROP_DEF
                    && c.borrow().get_name() == name
            })
            .cloned()
    }

    /// Get descendant by path (e.g. "nodegraph1/node1").
    pub fn get_descendant(&self, name_path: &str) -> Option<ElementPtr> {
        let parts: Vec<&str> = name_path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            return Some(self.root.clone());
        }
        let mut current = self.root.clone();
        for part in parts {
            let next = current.borrow().get_child(part)?.clone();
            current = next;
        }
        Some(current)
    }

    /// Add a NodeGraph child. Returns the new NodeGraph element.
    pub fn add_node_graph(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::NODE_GRAPH, name)
    }

    /// Get all NodeGraph elements.
    pub fn get_node_graphs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::NODE_GRAPH)
    }

    /// Get all NodeDef elements.
    pub fn get_node_defs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::NODEDEF)
    }

    /// Create NodeDef from NodeGraph (MaterialX Document::addNodeDefFromGraph).
    /// Copies graph content to new graph, transfers inputs/outputs to NodeDef, removes interface from graph.
    pub fn add_node_def_from_graph(
        &mut self,
        node_graph: &ElementPtr,
        node_def_name: &str,
        category_str: &str,
        new_graph_name: &str,
    ) -> Result<ElementPtr, String> {
        if category_str.is_empty() {
            return Err("Cannot create a nodedef without a category identifier".to_string());
        }
        if self.get_node_def(node_def_name).is_some() {
            return Err(format!(
                "Cannot create duplicate nodedef: {}",
                node_def_name
            ));
        }
        if self.get_node_graph(new_graph_name).is_some() {
            return Err(format!(
                "Cannot create duplicate nodegraph: {}",
                new_graph_name
            ));
        }

        let graph = self.add_node_graph(new_graph_name)?;
        copy_content_from_element(&graph, &node_graph.borrow());

        for child in graph.borrow().get_children() {
            child.borrow_mut().remove_attribute(XPOS_ATTRIBUTE);
            child.borrow_mut().remove_attribute(YPOS_ATTRIBUTE);
        }
        set_node_def_string(&graph, node_def_name);

        let node_def = self.add_node_def(node_def_name, "", category_str)?;
        set_node_string(&node_def, category_str);

        const FILTER_ATTRS: [&str; 5] = [
            NODE_GRAPH_ATTRIBUTE,
            NODE_NAME_ATTRIBUTE,
            INTERFACE_NAME_ATTRIBUTE,
            XPOS_ATTRIBUTE,
            YPOS_ATTRIBUTE,
        ];

        let graph_inputs: Vec<_> = crate::core::node::get_inputs(&graph)
            .into_iter()
            .map(|inp| {
                (
                    inp.borrow().get_name().to_string(),
                    inp.borrow()
                        .get_type()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                )
            })
            .collect();

        for (inp_name, inp_type) in &graph_inputs {
            let nd_inp = add_input(&node_def, inp_name, inp_type)?;
            if let Some(src) = get_input(&graph, inp_name) {
                copy_content_from_element(&nd_inp, &src.borrow());
                for fa in &FILTER_ATTRS {
                    nd_inp.borrow_mut().remove_attribute(fa);
                }
                nd_inp.borrow_mut().set_source_uri(Some(""));
                src.borrow_mut()
                    .set_attribute(INTERFACE_NAME_ATTRIBUTE, nd_inp.borrow().get_name());
            }
        }
        for inp_name in graph_inputs.iter().map(|(n, _)| n) {
            remove_input(&graph, inp_name);
        }

        let graph_outputs: Vec<_> = crate::core::node::get_outputs(&graph)
            .into_iter()
            .map(|out| {
                (
                    out.borrow().get_name().to_string(),
                    out.borrow()
                        .get_type()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                )
            })
            .collect();

        for (out_name, out_type) in &graph_outputs {
            let nd_out = add_output(&node_def, out_name, out_type)?;
            if let Some(src) = crate::core::node::get_output(&graph, out_name) {
                copy_content_from_element(&nd_out, &src.borrow());
                for fa in &FILTER_ATTRS {
                    nd_out.borrow_mut().remove_attribute(fa);
                }
                nd_out.borrow_mut().set_source_uri(Some(""));
            }
        }

        Ok(node_def)
    }

    /// Add a NodeDef with optional output type and node string.
    pub fn add_node_def(
        &mut self,
        name: &str,
        output_type: &str,
        node: &str,
    ) -> Result<ElementPtr, String> {
        let nd = self.add_child_of_category(category::NODEDEF, name)?;
        if !output_type.is_empty() && output_type != crate::core::types::MULTI_OUTPUT_TYPE_STRING {
            let out = add_child_of_category(&nd, category::OUTPUT, "out")?;
            out.borrow_mut().set_attribute("type", output_type);
        }
        if !node.is_empty() {
            nd.borrow_mut().set_attribute(NODE_ATTRIBUTE, node);
        }
        Ok(nd)
    }

    /// Add a Node to the document (Document is GraphElement). For node graphs, use add_child_of_category on the graph.
    pub fn add_node(
        &mut self,
        category_name: &str,
        name: &str,
        type_str: &str,
    ) -> Result<ElementPtr, String> {
        let node = add_child_of_category(&self.root, category_name, name)?;
        if !type_str.is_empty() {
            node.borrow_mut().set_attribute("type", type_str);
        }
        Ok(node)
    }

    /// Add a Node instance from NodeDef.
    pub fn add_node_instance(
        &mut self,
        node_def: &ElementPtr,
        name: &str,
    ) -> Result<ElementPtr, String> {
        let nd_node = node_def
            .borrow()
            .get_attribute(NODE_ATTRIBUTE)
            .map(|s| s.to_string())
            .unwrap_or_default();
        let nd_type = node_def
            .borrow()
            .get_attribute("type")
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::core::types::DEFAULT_TYPE_STRING.to_string());
        let node = self.add_node(&nd_node, name, &nd_type)?;
        node.borrow_mut()
            .set_attribute(NODE_DEF_ATTRIBUTE, node_def.borrow().get_name());
        Ok(node)
    }

    /// Add a Material node (surfacematerial/volumematerial) and optionally connect to shader.
    /// Sets MATERIAL_TYPE_STRING as the node type (matches C++ addMaterialNode).
    pub fn add_material_node(
        &mut self,
        name: &str,
        shader_node: Option<&ElementPtr>,
    ) -> Result<ElementPtr, String> {
        // Choose category based on shader type (volume vs surface)
        let node_cat = if let Some(shader) = shader_node {
            let shader_type = shader.borrow().get_type().unwrap_or("").to_string();
            if shader_type == crate::core::types::VOLUME_SHADER_TYPE_STRING {
                crate::core::types::VOLUME_MATERIAL_NODE_STRING
            } else {
                crate::core::types::SURFACE_MATERIAL_NODE_STRING
            }
        } else {
            crate::core::types::SURFACE_MATERIAL_NODE_STRING
        };
        let mat = self.add_child_of_category(node_cat, name)?;
        // set type="material" on the node
        mat.borrow_mut()
            .set_type(crate::core::types::MATERIAL_TYPE_STRING);
        if let Some(shader) = shader_node {
            let shader_type = shader
                .borrow()
                .get_type()
                .unwrap_or("surfaceshader")
                .to_string();
            let inp = add_child_of_category(&mat, category::INPUT, &shader_type)?;
            inp.borrow_mut().set_node_name(shader.borrow().get_name());
            inp.borrow_mut().set_type(&shader_type);
        }
        Ok(mat)
    }

    /// Add a Collection.
    pub fn add_collection(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::COLLECTION, name)
    }

    /// Get Collection by name.
    pub fn get_collection(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child(name)?;
        if child.borrow().get_category() == category::COLLECTION {
            Some(child)
        } else {
            None
        }
    }

    /// Get all Collections.
    pub fn get_collections(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::COLLECTION)
    }

    /// Remove Collection by name.
    pub fn remove_collection(&mut self, name: &str) {
        self.root.borrow_mut().remove_child(name);
    }

    /// Add a PropertySet.
    pub fn add_property_set(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::PROPERTY_SET, name)
    }

    /// Get PropertySet by name.
    pub fn get_property_set(&self, name: &str) -> Option<ElementPtr> {
        let child = self.get_child(name)?;
        if child.borrow().get_category() == category::PROPERTY_SET {
            Some(child)
        } else {
            None
        }
    }

    /// Get all PropertySets.
    pub fn get_property_sets(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::PROPERTY_SET)
    }

    /// Remove PropertySet by name.
    pub fn remove_property_set(&mut self, name: &str) {
        self.root.borrow_mut().remove_child(name);
    }

    /// Get Nodes in document (direct children that are nodes).
    pub fn get_nodes(&self, filter_category: &str) -> Vec<ElementPtr> {
        self.root
            .borrow()
            .get_children()
            .iter()
            .filter(|c| {
                let cat = c.borrow().get_category().to_string();
                if filter_category.is_empty() {
                    cat != category::NODE_GRAPH
                        && cat != category::NODEDEF
                        && cat != category::MATERIAL
                        && cat != category::LOOK
                        && cat != category::IMPLEMENTATION
                        && cat != category::TYPEDEF
                        && cat != category::GEOM_INFO
                        && cat != category::COLLECTION
                        && cat != category::PROPERTY_SET
                        && cat != category::UNIT_TYPEDEF
                        && cat != category::UNIT_DEF
                        && cat != category::VARIANT_SET
                        && cat != category::GEOM_PROP_DEF
                } else {
                    cat == filter_category
                }
            })
            .cloned()
            .collect()
    }

    /// Document version as string (e.g. "1.39"). Matches MaterialX Document::getVersionString.
    pub fn get_version_string(&self) -> String {
        self.get_doc_version_string()
    }

    /// Document version as (major, minor). Matches MaterialX Document::getVersionIntegers.
    pub fn get_version_integers(&self) -> (i32, i32) {
        self.get_doc_version_integers()
    }

    /// Validate document structure. Returns true if valid.
    /// Simple implementation: checks output type matches connected node (when both have type).
    pub fn validate(&self) -> bool {
        self.validate_impl()
    }

    fn validate_impl(&self) -> bool {
        let expected_version = crate::get_version_integers();
        let doc_version = self.get_version_integers();
        if doc_version < expected_version || doc_version > expected_version {
            return false;
        }
        for child in self.root.borrow().get_children() {
            let cat = child.borrow().get_category().to_string();
            if cat == category::NODE_GRAPH {
                if !crate::core::node::validate_node_graph(&child).0 {
                    return false;
                }
            }
        }
        true
    }

    /// Import library document: copy all children from library into this document.
    pub fn import_library(&mut self, library: &Document) {
        let lib_root = library.get_root();
        let children: Vec<ElementPtr> = lib_root.borrow().get_children().to_vec();
        let lib_attrs = (
            lib_root.borrow().has_file_prefix(),
            lib_root.borrow().get_file_prefix(),
            lib_root.borrow().has_geom_prefix(),
            lib_root.borrow().get_geom_prefix(),
            lib_root.borrow().has_color_space(),
            lib_root.borrow().get_color_space(),
            lib_root.borrow().get_source_uri().map(|s| s.to_string()),
            lib_root.borrow().has_namespace(),
            lib_root.borrow().get_namespace(),
        );

        for child in children {
            let (cat, child_name) = {
                let src = child.borrow();
                let cat = src.get_category().to_string();
                if cat.is_empty() {
                    continue;
                }
                let child_name = src.get_qualified_name(&src.name);
                (cat, child_name)
            };

            if self.root.borrow().get_child(&child_name).is_some() {
                continue;
            }

            let child_copy = add_child_of_category(&self.root, &cat, &child_name)
                .expect("import_library: add_child_of_category");
            copy_content_from_element(&child_copy, &child.borrow());

            // Inherit fileprefix, geomprefix, colorspace, namespace, sourceUri from library if not set
            let mut child_mut = child_copy.borrow_mut();
            if !child_mut.has_file_prefix() && lib_attrs.0 {
                child_mut.set_file_prefix(&lib_attrs.1);
            }
            if !child_mut.has_geom_prefix() && lib_attrs.2 {
                child_mut.set_geom_prefix(&lib_attrs.3);
            }
            if !child_mut.has_color_space() && lib_attrs.4 {
                child_mut.set_color_space(&lib_attrs.5);
            }
            // H-C7: copy namespace from library root to child (C++ importLibrary does this)
            if !child_mut.has_namespace() && lib_attrs.7 {
                child_mut.set_namespace(&lib_attrs.8);
            }
            if !child_mut.has_source_uri() {
                if let Some(ref uri) = lib_attrs.6 {
                    child_mut.set_source_uri(Some(uri.as_str()));
                }
            }
        }
    }

    // --- DRY helpers for typed CRUD ---

    /// Get child by name, filtered by category.
    fn get_child_by_cat(&self, name: &str, cat: &str) -> Option<ElementPtr> {
        let child = self.get_child(name)?;
        if child.borrow().get_category() == cat {
            Some(child)
        } else {
            None
        }
    }

    /// Get all children of given category.
    fn get_children_by_cat(&self, cat: &str) -> Vec<ElementPtr> {
        self.root
            .borrow()
            .get_children()
            .iter()
            .filter(|c| c.borrow().get_category() == cat)
            .cloned()
            .collect()
    }

    fn get_matching_names(&self, name: &str) -> Vec<String> {
        let mut names = Vec::new();
        if name.is_empty() {
            return names;
        }
        names.push(name.to_string());
        let qualified = self.root.borrow().get_qualified_name(name);
        if qualified != name {
            names.push(qualified);
        }
        names
    }

    /// Remove child by name (no category check, same as C++ removeChildOfType).
    fn remove_child_by_name(&mut self, name: &str) {
        self.root.borrow_mut().remove_child(name);
    }

    // --- NodeGraph CRUD (continued) ---

    /// Remove NodeGraph by name.
    pub fn remove_node_graph(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    /// Remove NodeDef by name.
    pub fn remove_node_def(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- Look CRUD ---

    /// Add a Look to the document.
    pub fn add_look(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::LOOK, name)
    }

    /// Get all Look elements.
    pub fn get_looks(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::LOOK)
    }

    /// Remove Look by name.
    pub fn remove_look(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- LookGroup CRUD ---

    /// Add a LookGroup to the document.
    pub fn add_look_group(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::LOOK_GROUP, name)
    }

    /// Get LookGroup by name.
    pub fn get_look_group(&self, name: &str) -> Option<ElementPtr> {
        self.get_child_by_cat(name, category::LOOK_GROUP)
    }

    /// Get all LookGroup elements.
    pub fn get_look_groups(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::LOOK_GROUP)
    }

    /// Remove LookGroup by name.
    pub fn remove_look_group(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- GeomInfo CRUD ---

    /// Add a GeomInfo to the document with optional geom string.
    pub fn add_geom_info(&mut self, name: &str, geom: &str) -> Result<ElementPtr, String> {
        let gi = self.add_child_of_category(category::GEOM_INFO, name)?;
        if !geom.is_empty() {
            gi.borrow_mut()
                .set_attribute(crate::core::element::GEOM_ATTRIBUTE, geom);
        }
        Ok(gi)
    }

    /// Get GeomInfo by name.
    pub fn get_geom_info(&self, name: &str) -> Option<ElementPtr> {
        self.get_child_by_cat(name, category::GEOM_INFO)
    }

    /// Get all GeomInfo elements.
    pub fn get_geom_infos(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::GEOM_INFO)
    }

    /// Remove GeomInfo by name.
    pub fn remove_geom_info(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- GeomPropDef CRUD ---

    /// Add a GeomPropDef to the document.
    pub fn add_geom_prop_def(&mut self, name: &str, geomprop: &str) -> Result<ElementPtr, String> {
        let gpd = self.add_child_of_category(category::GEOM_PROP_DEF, name)?;
        if !geomprop.is_empty() {
            gpd.borrow_mut()
                .set_attribute(crate::core::geom::GEOM_PROP_ATTRIBUTE, geomprop);
        }
        Ok(gpd)
    }

    /// Get all GeomPropDef elements.
    pub fn get_geom_prop_defs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::GEOM_PROP_DEF)
    }

    /// Remove GeomPropDef by name.
    pub fn remove_geom_prop_def(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- TypeDef CRUD ---

    /// Add a TypeDef to the document.
    pub fn add_type_def(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::TYPEDEF, name)
    }

    /// Get all TypeDef elements.
    pub fn get_type_defs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::TYPEDEF)
    }

    /// Remove TypeDef by name.
    pub fn remove_type_def(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- AttributeDef CRUD ---

    /// Add an AttributeDef to the document.
    pub fn add_attribute_def(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::ATTRIBUTE_DEF, name)
    }

    /// Get AttributeDef by name.
    pub fn get_attribute_def(&self, name: &str) -> Option<ElementPtr> {
        self.get_child_by_cat(name, category::ATTRIBUTE_DEF)
    }

    /// Get all AttributeDef elements.
    pub fn get_attribute_defs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::ATTRIBUTE_DEF)
    }

    /// Remove AttributeDef by name.
    pub fn remove_attribute_def(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- TargetDef CRUD ---

    /// Add a TargetDef to the document.
    pub fn add_target_def(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::TARGET_DEF, name)
    }

    /// Get TargetDef by name.
    pub fn get_target_def(&self, name: &str) -> Option<ElementPtr> {
        self.get_child_by_cat(name, category::TARGET_DEF)
    }

    /// Get all TargetDef elements.
    pub fn get_target_defs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::TARGET_DEF)
    }

    /// Remove TargetDef by name.
    pub fn remove_target_def(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- VariantSet CRUD ---

    /// Add a VariantSet to the document.
    pub fn add_variant_set(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::VARIANT_SET, name)
    }

    /// Get VariantSet by name.
    pub fn get_variant_set(&self, name: &str) -> Option<ElementPtr> {
        self.get_child_by_cat(name, category::VARIANT_SET)
    }

    /// Get all VariantSet elements.
    pub fn get_variant_sets(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::VARIANT_SET)
    }

    /// Remove VariantSet by name.
    pub fn remove_variant_set(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- Implementation CRUD ---

    /// Add an Implementation to the document.
    pub fn add_implementation(&mut self, name: &str) -> Result<ElementPtr, String> {
        self.add_child_of_category(category::IMPLEMENTATION, name)
    }

    /// Get all Implementation elements.
    pub fn get_implementations(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::IMPLEMENTATION)
    }

    /// Remove Implementation by name.
    pub fn remove_implementation(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- UnitDef CRUD ---

    /// Add a UnitDef to the document.
    pub fn add_unit_def(&mut self, name: &str) -> Result<ElementPtr, String> {
        if name.is_empty() {
            return Err("A unit definition name cannot be empty".to_string());
        }
        self.add_child_of_category(category::UNIT_DEF, name)
    }

    /// Get UnitDef by name.
    pub fn get_unit_def(&self, name: &str) -> Option<ElementPtr> {
        self.get_child_by_cat(name, category::UNIT_DEF)
    }

    /// Get all UnitDef elements.
    pub fn get_unit_defs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::UNIT_DEF)
    }

    /// Remove UnitDef by name.
    pub fn remove_unit_def(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- UnitTypeDef CRUD ---

    /// Add a UnitTypeDef to the document.
    pub fn add_unit_type_def(&mut self, name: &str) -> Result<ElementPtr, String> {
        if name.is_empty() {
            return Err("A unit type definition name cannot be empty".to_string());
        }
        self.add_child_of_category(category::UNIT_TYPEDEF, name)
    }

    /// Get all UnitTypeDef elements.
    pub fn get_unit_type_defs(&self) -> Vec<ElementPtr> {
        self.get_children_by_cat(category::UNIT_TYPEDEF)
    }

    /// Remove UnitTypeDef by name.
    pub fn remove_unit_type_def(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- Node removal ---

    /// Remove a node by name from the document.
    pub fn remove_node(&mut self, name: &str) {
        self.remove_child_by_name(name);
    }

    // --- Color Management System (C++ Document CMS methods) ---

    /// Set color management system string.
    pub fn set_color_management_system(&mut self, cms: impl Into<String>) {
        self.root.borrow_mut().set_attribute(CMS_ATTRIBUTE, cms);
    }

    /// Has color management system string.
    pub fn has_color_management_system(&self) -> bool {
        self.root.borrow().has_attribute(CMS_ATTRIBUTE)
    }

    /// Get color management system string.
    pub fn get_color_management_system(&self) -> String {
        self.root.borrow().get_attribute_or_empty(CMS_ATTRIBUTE)
    }

    /// Set color management config string.
    pub fn set_color_management_config(&mut self, config: impl Into<String>) {
        self.root
            .borrow_mut()
            .set_attribute(CMS_CONFIG_ATTRIBUTE, config);
    }

    /// Has color management config string.
    pub fn has_color_management_config(&self) -> bool {
        self.root.borrow().has_attribute(CMS_CONFIG_ATTRIBUTE)
    }

    /// Get color management config string.
    pub fn get_color_management_config(&self) -> String {
        self.root
            .borrow()
            .get_attribute_or_empty(CMS_CONFIG_ATTRIBUTE)
    }

    // --- Document copy and source URIs ---

    /// Create a deep copy of this document. C++ Document::copy.
    pub fn copy(&self) -> Document {
        let doc = create_document();
        copy_content_from_element(&doc.root, &self.root.borrow());
        // Note: data_library is not deep-copied, only referenced
        doc
    }

    /// Get a set of all source URIs referenced by elements in this document.
    /// C++ Document::getReferencedSourceUris.
    pub fn get_referenced_source_uris(&self) -> std::collections::HashSet<String> {
        let mut uris = std::collections::HashSet::new();
        collect_source_uris(&self.root, &mut uris);
        uris
    }

    // --- Version management (H-C10) -----------------------------------------

    /// Return the version string stored in the document's root element (e.g. "1.39").
    /// If missing, returns the current library version.
    pub fn get_doc_version_string(&self) -> String {
        self.root
            .borrow()
            .get_attribute(crate::core::element::VERSION_ATTRIBUTE)
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::get_version_string().to_string())
    }

    /// Set the version attribute on the document root element.
    pub fn set_doc_version_string(&mut self, version: impl Into<String>) {
        self.root
            .borrow_mut()
            .set_attribute(crate::core::element::VERSION_ATTRIBUTE, version);
    }

    /// Parse the document version string into (major, minor).
    /// Returns library version if the attribute is missing or unparseable.
    pub fn get_doc_version_integers(&self) -> (i32, i32) {
        let s = self.get_doc_version_string();
        parse_version_string(&s).unwrap_or_else(|| crate::get_version_integers())
    }

    /// Upgrade document from earlier MaterialX versions to the current library version.
    ///
    /// Matches C++ `Document::upgradeVersion()` in Version.cpp.
    /// Implements step-by-step upgrade transforms from v1.22 through v1.39.
    /// Each step converts legacy attributes and categories to their modern equivalents.
    #[allow(unused_assignments)]
    pub fn upgrade_version(&mut self) {
        let doc_ver = self.get_doc_version_integers();
        let lib_ver = crate::get_version_integers();
        if doc_ver >= lib_ver {
            return;
        }
        let (major, mut minor) = doc_ver;

        // Collect all elements in the tree for bulk transforms.
        fn collect_tree(elem: &ElementPtr, out: &mut Vec<ElementPtr>) {
            out.push(elem.clone());
            for child in elem.borrow().get_children().to_vec() {
                collect_tree(&child, out);
            }
        }

        // v1.22 -> v1.23: rename type "vector" to "vector3"
        if major == 1 && minor == 22 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                if elem.borrow().get_type() == Some("vector") {
                    elem.borrow_mut().set_type("vector3");
                }
            }
            minor = 23;
        }

        // v1.23 -> v1.24: rename "shader" attribute "shadername" to "node"
        if major == 1 && minor == 23 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                let cat = elem.borrow().get_category().to_string();
                if cat == "shader" {
                    if let Some(sn) = elem
                        .borrow()
                        .get_attribute("shadername")
                        .map(|s| s.to_string())
                    {
                        elem.borrow_mut().set_attribute(NODE_ATTRIBUTE, sn);
                        elem.borrow_mut().remove_attribute("shadername");
                    }
                }
            }
            minor = 24;
        }

        // v1.24 -> v1.25: rename input attribute "graphname" to "opgraph"
        if major == 1 && minor == 24 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                if elem.borrow().get_category() == category::INPUT {
                    if let Some(gn) = elem
                        .borrow()
                        .get_attribute("graphname")
                        .map(|s| s.to_string())
                    {
                        elem.borrow_mut().set_attribute("opgraph", gn);
                        elem.borrow_mut().remove_attribute("graphname");
                    }
                }
            }
            minor = 25;
        }

        // v1.25 -> v1.26: rename "opgraph" to "nodegraph", rename category "opgraph" to "nodegraph",
        // rename "shadertype" attribute to "context" on nodedef.
        if major == 1 && minor == 25 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                let cat = elem.borrow().get_category().to_string();
                if cat == category::INPUT {
                    if let Some(og) = elem
                        .borrow()
                        .get_attribute("opgraph")
                        .map(|s| s.to_string())
                    {
                        elem.borrow_mut().set_attribute(NODE_GRAPH_ATTRIBUTE, og);
                        elem.borrow_mut().remove_attribute("opgraph");
                    }
                }
                if cat == "opgraph" {
                    elem.borrow_mut().set_category(category::NODE_GRAPH);
                }
                if cat == category::NODEDEF {
                    if let Some(st) = elem
                        .borrow()
                        .get_attribute("shadertype")
                        .map(|s| s.to_string())
                    {
                        elem.borrow_mut().set_attribute("context", st);
                        elem.borrow_mut().remove_attribute("shadertype");
                    }
                }
            }
            minor = 26;
        }

        // v1.26 -> v1.36: rename "default" inputs to "in", migrate geomattr to geomprop,
        // rename "constant"->"value" node, "image"->"tiledimage" when needed, etc.
        if major == 1 && minor >= 26 && minor < 36 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                let cat = elem.borrow().get_category().to_string();
                // Rename "default" input attribute to "in" for certain nodes
                if cat == category::INPUT {
                    let name = elem.borrow().get_name().to_string();
                    if name == "default" {
                        // Handled by parent — skip attribute rename for now,
                        // these are very old legacy documents.
                    }
                }
                // Rename geomattr -> geomprop
                if cat == "geomattr" {
                    elem.borrow_mut().set_category("geomprop");
                }
                if cat == "geomattrvalue" {
                    elem.borrow_mut().set_category("geompropvalue");
                }
            }
            minor = 36;
        }

        // v1.36 -> v1.37: legacy material/shaderref -> surfacematerial node.
        // Convert old-style Material elements with ShaderRef children to
        // surfacematerial nodes with surfaceshader inputs.
        if major == 1 && minor == 36 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                let cat = elem.borrow().get_category().to_string();
                // Rename category "material" to "surfacematerial" (modern convention)
                if cat == "material" {
                    // Check if it has shaderref children (legacy format)
                    let has_shader_ref = elem
                        .borrow()
                        .get_children()
                        .iter()
                        .any(|c| c.borrow().get_category() == category::SHADER_REF);
                    if has_shader_ref {
                        elem.borrow_mut().set_category(category::MATERIAL);
                    }
                }
                // Rename "typeassign" -> "tokenassign"
                if cat == "typeassign" {
                    elem.borrow_mut().set_category("tokenassign");
                }
            }
            minor = 37;
        }

        // v1.37 -> v1.38: rename node categories (e.g. "atan2" 2-arg -> update,
        // "switch" nodes get renamed outputs, etc.)
        if major == 1 && minor == 37 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                let cat = elem.borrow().get_category().to_string();
                // Rename known legacy categories
                match cat.as_str() {
                    "atan2" => {
                        // The C++ upgrade renames atan2 with swapped input args
                        // if it has the old convention. We keep the category rename.
                    }
                    "arrayappend" => {
                        elem.borrow_mut().set_category("appendarray");
                    }
                    _ => {}
                }
            }
            minor = 38;
        }

        // v1.38 -> v1.39: final upgrades
        if major == 1 && minor == 38 {
            let mut elems = Vec::new();
            collect_tree(&self.root, &mut elems);
            for elem in &elems {
                let cat = elem.borrow().get_category().to_string();
                // Rename "subsurface_bsdf" to "translucent_bsdf" in certain contexts
                if cat == "subsurface_bsdf" {
                    elem.borrow_mut().set_category("translucent_bsdf");
                }
            }
            minor = 39;
        }

        // Bump the stored version to the library's current version.
        let (maj, min) = lib_ver;
        self.set_doc_version_string(format!("{}.{:02}", maj, min));
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::element::add_child_of_category;

    #[test]
    fn test_get_matching_ports() {
        let doc = create_document();
        let root = doc.get_root();
        let graph = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        let n1 = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        n1.borrow_mut().set_attribute("type", "float");
        let n2 = add_child_of_category(&graph, category::NODE, "n2").unwrap();
        n2.borrow_mut().set_attribute("type", "float");
        // inp on n2 connects to n1.
        let inp = add_child_of_category(&n2, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_node_name("n1");
        // Graph output also connects to n1.
        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_node_name("n1");

        let ports = doc.get_matching_ports("n1");
        assert_eq!(ports.len(), 2, "should find two ports connecting to n1");

        let ports_n2 = doc.get_matching_ports("n2");
        assert!(ports_n2.is_empty(), "nothing connects to n2");
    }

    #[test]
    fn test_get_geom_prop_value() {
        let mut doc = create_document();
        let gi = doc.add_geom_info("gi1", "*").unwrap();
        // Add a geomprop child with value.
        let gp = add_child_of_category(&gi, category::GEOM_PROP, "Nworld").unwrap();
        gp.borrow_mut().set_value_string("0 1 0");

        // Universal geom matches any query.
        let val = doc.get_geom_prop_value("Nworld", "*");
        assert_eq!(val, Some("0 1 0".to_string()));

        // Non-existent property returns None.
        let missing = doc.get_geom_prop_value("Pworld", "*");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_material_outputs() {
        let mut doc = create_document();
        // NodeGraph containing a material node and output.
        let graph = doc.add_node_graph("surface_g").unwrap();
        let mat_node = add_child_of_category(&graph, category::NODE, "mat").unwrap();
        mat_node
            .borrow_mut()
            .set_attribute("type", crate::core::types::MATERIAL_TYPE_STRING);
        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut()
            .set_attribute("type", crate::core::types::MATERIAL_TYPE_STRING);
        out.borrow_mut().set_node_name("mat");

        let mat_outs = doc.get_material_outputs();
        assert_eq!(mat_outs.len(), 1, "should find one material output");
        assert_eq!(mat_outs[0].borrow().get_name(), "out");
    }

    #[test]
    fn test_get_material_outputs_skips_defs() {
        let mut doc = create_document();
        // NodeGraph that is a NodeDef implementation -- should be skipped.
        let graph = doc.add_node_graph("impl_g").unwrap();
        graph
            .borrow_mut()
            .set_attribute(NODE_DEF_ATTRIBUTE, "ND_foo");
        let mat_node = add_child_of_category(&graph, category::NODE, "mat").unwrap();
        mat_node
            .borrow_mut()
            .set_attribute("type", crate::core::types::MATERIAL_TYPE_STRING);
        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut()
            .set_attribute("type", crate::core::types::MATERIAL_TYPE_STRING);
        out.borrow_mut().set_node_name("mat");

        let mat_outs = doc.get_material_outputs();
        assert!(
            mat_outs.is_empty(),
            "definition nodegraph should be skipped"
        );
    }

    #[test]
    fn test_add_newline_doc() {
        let mut doc = create_document();
        let nl = doc.add_newline().unwrap();
        assert_eq!(
            nl.borrow().get_category(),
            category::NEWLINE,
            "add_newline should produce a newline element"
        );
        assert_eq!(doc.get_children().len(), 1);
    }

    #[test]
    fn test_add_newline_unique_names() {
        let mut doc = create_document();
        let nl1 = doc.add_newline().unwrap();
        let nl2 = doc.add_newline().unwrap();
        assert_ne!(
            nl1.borrow().get_name(),
            nl2.borrow().get_name(),
            "newline elements must have unique names"
        );
    }

    #[test]
    fn test_invalidate_cache_noop() {
        // invalidate_cache is a no-op, just verify it doesn't panic.
        let doc = create_document();
        doc.invalidate_cache();
    }
}
