//! Bridge from mtlx-rs Document to schema::mtlx Document.
//!
//! When the `mtlx-rs` feature is enabled, MaterialX files can be parsed using
//! the full MaterialX library (mtlx-rs) with proper XInclude resolution,
//! FileSearchPath, and cycle detection. The mtlx-rs Document is converted to
//! the schema::mtlx arena-based Document for use with usd_mtlx_read.
//!
//! # Example
//!
//! ```ignore
//! use usd::schema::mtlx::{document_from_mtlx_rs, usd_mtlx_read};
//! use mtlx_rs::format::read_from_xml_file;
//!
//! let mtlx_doc = read_from_xml_file("scene.mtlx").expect("parse");
//! let schema_doc = document_from_mtlx_rs(&mtlx_doc);
//! usd_mtlx_read(&schema_doc, &stage, ...);
//! ```

use super::document::{Document, DocumentData, ElementData};

/// Convert mtlx-rs Document to schema::mtlx Document.
///
/// Traverses the mtlx-rs element tree and builds the arena-based Document
/// structure used by usd_mtlx_read.
///
/// # Feature
///
/// Only available when the `mtlx-rs` feature is enabled.
pub fn document_from_mtlx_rs(mtlx_doc: &mtlx_rs::core::Document) -> Document {
    let mut data = DocumentData {
        elements: Vec::new(),
        root_idx: 0,
    };

    let root_ptr = mtlx_doc.get_root();
    let root_elem = root_ptr.borrow();

    let default_uri = root_elem.get_source_uri().unwrap_or("").to_string();
    let root_idx = convert_element(&root_elem, &mut data, None, &default_uri);
    data.root_idx = root_idx;

    Document::from_data(data)
}

fn convert_element(
    mtlx_elem: &mtlx_rs::core::Element,
    data: &mut DocumentData,
    parent_idx: Option<usize>,
    default_source_uri: &str,
) -> usize {
    let name = mtlx_elem.get_name().to_string();
    let category = mtlx_elem.get_category().to_string();

    let mut elem_data = ElementData::new(name, category);

    for (k, v) in mtlx_elem.iter_attributes() {
        elem_data.attributes.insert(k.to_string(), v.to_string());
    }

    elem_data.parent = parent_idx;
    elem_data.source_uri = mtlx_elem
        .get_source_uri()
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_source_uri.to_string());

    let my_idx = data.elements.len();
    data.elements.push(elem_data);

    for child_ptr in mtlx_elem.get_children() {
        let child_elem = child_ptr.borrow();
        let child_source = child_elem
            .get_source_uri()
            .map(|s| s.to_string())
            .unwrap_or_else(|| default_source_uri.to_string());
        let child_idx = convert_element(&child_elem, data, Some(my_idx), &child_source);
        data.elements[my_idx].children.push(child_idx);
    }

    my_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_from_mtlx_rs() {
        let mtlx_doc = mtlx_rs::core::document::create_document();
        let schema_doc = document_from_mtlx_rs(&mtlx_doc);
        let schema_root = schema_doc.get_root();
        assert_eq!(schema_root.category(), "materialx");
    }
}
