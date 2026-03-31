//! MaterialX XML I/O - parse .mtlx XML files into Document structures.
//!
//! This module provides XML parsing using quick_xml, handling XInclude
//! directives for library composition.

use crate::document::{Document, DocumentData, ElementData, MtlxError};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::HashMap;
use std::path::Path;

/// Parse MaterialX document from XML file
pub fn read_from_xml_file<P: AsRef<Path>>(path: P) -> Result<Document, MtlxError> {
    let path = path.as_ref();
    let xml_content = std::fs::read_to_string(path)
        .map_err(|e| MtlxError::new(format!("Failed to read file '{}': {}", path.display(), e)))?;

    let source_uri = path
        .to_str()
        .ok_or_else(|| MtlxError::new("Invalid UTF-8 in file path"))?
        .to_string();

    read_from_xml_string_internal(
        &xml_content,
        source_uri,
        Some(path.parent().unwrap_or(path)),
    )
}

/// Parse MaterialX document from XML string
pub fn read_from_xml_string(xml: &str) -> Result<Document, MtlxError> {
    // C++ mx::readFromXmlString would throw on empty input; match that behaviour
    if xml.trim().is_empty() {
        return Err(MtlxError::new("Empty MaterialX XML string"));
    }
    read_from_xml_string_internal(xml, String::new(), None)
}

/// Internal XML parsing with source URI tracking
fn read_from_xml_string_internal(
    xml: &str,
    source_uri: String,
    base_path: Option<&Path>,
) -> Result<Document, MtlxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut data = DocumentData::new();
    data.elements[0].source_uri = source_uri.clone();

    let mut elem_stack: Vec<usize> = vec![];
    let mut buf = Vec::new();
    let mut is_first_element = true;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                // Handle XInclude
                if e.name().as_ref() == b"xi:include" || e.name().as_ref() == b"include" {
                    if let Some(href) = get_attribute(&e, "href") {
                        if let Some(base) = base_path {
                            let include_path = base.join(&href);
                            if let Ok(included_doc) = read_from_xml_file(&include_path) {
                                // Merge included document
                                let parent_idx =
                                    elem_stack.last().copied().unwrap_or(data.root_idx);
                                merge_document(&mut data, &included_doc, parent_idx);
                            }
                        }
                    }
                    buf.clear();
                    continue;
                }

                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let name = get_attribute(&e, "name").unwrap_or_default();

                // First element is the document root (usually <materialx>)
                if is_first_element {
                    is_first_element = false;

                    // Update existing root element
                    data.elements[0].category = tag_name;
                    data.elements[0].name = name;

                    // Parse attributes
                    for attr_result in e.attributes() {
                        if let Ok(attr) = attr_result {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key != "name" {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                data.elements[0].attributes.insert(key, value);
                            }
                        }
                    }

                    elem_stack.push(data.root_idx);
                    buf.clear();
                    continue;
                }

                // Parse child element
                let mut element = ElementData::new(name, tag_name);
                element.source_uri = source_uri.clone();
                element.parent = elem_stack.last().copied();

                // Parse attributes
                for attr_result in e.attributes() {
                    if let Ok(attr) = attr_result {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        if key != "name" {
                            let value = String::from_utf8_lossy(&attr.value).to_string();
                            element.attributes.insert(key, value);
                        }
                    }
                }

                // Add element to document
                let elem_idx = data.add_element(element);

                // Add to parent's children
                if let Some(&parent_idx) = elem_stack.last() {
                    data.elements[parent_idx].children.push(elem_idx);
                }

                // Push to stack (not empty tag)
                elem_stack.push(elem_idx);
            }
            Ok(Event::Empty(e)) => {
                // Handle XInclude
                if e.name().as_ref() == b"xi:include" || e.name().as_ref() == b"include" {
                    if let Some(href) = get_attribute(&e, "href") {
                        if let Some(base) = base_path {
                            let include_path = base.join(&href);
                            if let Ok(included_doc) = read_from_xml_file(&include_path) {
                                // Merge included document
                                let parent_idx =
                                    elem_stack.last().copied().unwrap_or(data.root_idx);
                                merge_document(&mut data, &included_doc, parent_idx);
                            }
                        }
                    }
                    buf.clear();
                    continue;
                }

                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let name = get_attribute(&e, "name").unwrap_or_default();

                // First element is the document root
                if is_first_element {
                    is_first_element = false;

                    // Update existing root element
                    data.elements[0].category = tag_name;
                    data.elements[0].name = name;

                    // Parse attributes
                    for attr_result in e.attributes() {
                        if let Ok(attr) = attr_result {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key != "name" {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                data.elements[0].attributes.insert(key, value);
                            }
                        }
                    }

                    // Don't push to stack (empty root)
                    buf.clear();
                    continue;
                }

                // Parse empty element
                let mut element = ElementData::new(name, tag_name);
                element.source_uri = source_uri.clone();
                element.parent = elem_stack.last().copied();

                // Parse attributes
                for attr_result in e.attributes() {
                    if let Ok(attr) = attr_result {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        if key != "name" {
                            let value = String::from_utf8_lossy(&attr.value).to_string();
                            element.attributes.insert(key, value);
                        }
                    }
                }

                // Add element to document
                let elem_idx = data.add_element(element);

                // Add to parent's children
                if let Some(&parent_idx) = elem_stack.last() {
                    data.elements[parent_idx].children.push(elem_idx);
                }

                // Don't push to stack (empty tag)
            }
            Ok(Event::End(_)) => {
                elem_stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(MtlxError::new(format!(
                    "XML parsing error at position {}: {}",
                    reader.buffer_position(),
                    e
                )));
            }
            _ => {}
        }

        buf.clear();
    }

    Ok(Document::from_data(data))
}

/// Get attribute value from BytesStart element
fn get_attribute(element: &BytesStart, name: &str) -> Option<String> {
    for attr_result in element.attributes() {
        if let Ok(attr) = attr_result {
            if attr.key.as_ref() == name.as_bytes() {
                return Some(String::from_utf8_lossy(&attr.value).to_string());
            }
        }
    }
    None
}

/// Merge included document into parent document
fn merge_document(target: &mut DocumentData, source: &Document, parent_idx: usize) {
    let source_data = source.inner();

    // Copy all elements except root from source
    let mut index_map: HashMap<usize, usize> = HashMap::new();

    for (src_idx, src_elem) in source_data.elements.iter().enumerate() {
        if src_idx == source_data.root_idx {
            // Map source root to target parent
            index_map.insert(src_idx, parent_idx);
            continue;
        }

        let mut new_elem = src_elem.clone();
        let new_idx = target.elements.len();
        index_map.insert(src_idx, new_idx);

        // Update parent reference
        if let Some(src_parent) = new_elem.parent {
            new_elem.parent = index_map.get(&src_parent).copied();
        }

        // Children will be updated in second pass
        new_elem.children.clear();

        target.elements.push(new_elem);
    }

    // Second pass: update children and add to parents
    for (src_idx, src_elem) in source_data.elements.iter().enumerate() {
        if src_idx == source_data.root_idx {
            continue;
        }

        let new_idx = index_map[&src_idx];

        // Update children
        for &src_child in &src_elem.children {
            if let Some(&new_child) = index_map.get(&src_child) {
                target.elements[new_idx].children.push(new_child);
            }
        }

        // Add to parent's children list
        if let Some(parent) = target.elements[new_idx].parent {
            if !target.elements[parent].children.contains(&new_idx) {
                target.elements[parent].children.push(new_idx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_document() {
        let xml = r#"
<?xml version="1.0"?>
<materialx version="1.38">
  <nodedef name="ND_test" node="test" type="color3">
    <input name="amount" type="float" value="0.5"/>
  </nodedef>
</materialx>
"#;

        let doc = read_from_xml_string(xml).unwrap();
        let root = doc.get_root();

        assert_eq!(root.category(), "materialx");
        assert_eq!(root.get_attribute("version"), "1.38");

        let nodedefs = doc.get_node_defs();
        assert_eq!(nodedefs.len(), 1);
        assert_eq!(nodedefs[0].0.name(), "ND_test");
        assert_eq!(nodedefs[0].get_node_string(), "test");
        assert_eq!(nodedefs[0].get_type(), "color3");

        let inputs = nodedefs[0].get_inputs();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].0.name(), "amount");
        assert_eq!(inputs[0].get_type(), "float");
        assert_eq!(inputs[0].get_value_string(), "0.5");
    }

    #[test]
    fn test_parse_nodegraph() {
        let xml = r#"
<?xml version="1.0"?>
<materialx>
  <nodegraph name="NG_test">
    <input name="in1" type="color3"/>
    <node name="multiply" node="multiply">
      <input name="in1" type="color3" interfacename="in1"/>
      <input name="in2" type="color3" value="0.5, 0.5, 0.5"/>
    </node>
    <output name="out" type="color3" nodename="multiply"/>
  </nodegraph>
</materialx>
"#;

        let doc = read_from_xml_string(xml).unwrap();
        let nodegraphs: Vec<_> = doc
            .get_root()
            .get_children_of_type("nodegraph")
            .into_iter()
            .map(|e| crate::document::NodeGraph(e))
            .collect();

        assert_eq!(nodegraphs.len(), 1);
        assert_eq!(nodegraphs[0].0.name(), "NG_test");

        let inputs = nodegraphs[0].get_inputs();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].0.name(), "in1");

        let nodes = nodegraphs[0].get_nodes("");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].0.name(), "multiply");

        let outputs = nodegraphs[0].get_outputs();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].0.name(), "out");
    }

    #[test]
    fn test_parse_material_and_look() {
        let xml = r#"
<?xml version="1.0"?>
<materialx>
  <material name="M_test">
    <shaderref name="SR_test" node="standard_surface"/>
  </material>
  <look name="hero">
    <materialassign name="MA1" material="M_test" geom="/mesh1"/>
  </look>
</materialx>
"#;

        let doc = read_from_xml_string(xml).unwrap();

        let materials: Vec<_> = doc
            .get_root()
            .get_children_of_type("material")
            .into_iter()
            .collect();
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0].name(), "M_test");

        let looks = doc.get_looks();
        assert_eq!(looks.len(), 1);
        assert_eq!(looks[0].get_name(), "hero");

        let assigns = looks[0].get_material_assigns();
        assert_eq!(assigns.len(), 1);
        assert_eq!(assigns[0].get_material(), "M_test");
    }

    #[test]
    fn test_empty_element() {
        let xml = r#"
<?xml version="1.0"?>
<materialx>
  <nodedef name="ND_test" node="test" type="float"/>
</materialx>
"#;

        let doc = read_from_xml_string(xml).unwrap();
        let nodedefs = doc.get_node_defs();
        assert_eq!(nodedefs.len(), 1);
        assert_eq!(nodedefs[0].get_inputs().len(), 0);
    }

    #[test]
    fn test_element_navigation() {
        let xml = r#"
<?xml version="1.0"?>
<materialx>
  <nodedef name="parent">
    <input name="child1" type="float"/>
    <input name="child2" type="color3"/>
  </nodedef>
</materialx>
"#;

        let doc = read_from_xml_string(xml).unwrap();
        let nodedef = doc.get_node_def("parent").unwrap();
        let inputs = nodedef.get_inputs();

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].0.get_parent().unwrap().name(), "parent");
        assert_eq!(inputs[0].0.get_name_path(), "parent/child1");
    }

    #[test]
    fn test_color_space() {
        let xml = r#"
<?xml version="1.0"?>
<materialx colorspace="lin_rec709">
  <nodedef name="ND_test" node="test">
    <input name="color_input" type="color3" colorspace="srgb_texture"/>
  </nodedef>
</materialx>
"#;

        let doc = read_from_xml_string(xml).unwrap();
        assert_eq!(doc.get_active_color_space(), "lin_rec709");

        let nodedef = doc.get_node_def("ND_test").unwrap();
        let inputs = nodedef.get_inputs();
        assert_eq!(inputs[0].get_color_space(), "srgb_texture");
        assert_eq!(inputs[0].get_active_color_space(), "srgb_texture");
    }
}
